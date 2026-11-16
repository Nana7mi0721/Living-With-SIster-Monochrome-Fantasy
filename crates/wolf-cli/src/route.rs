//! The main-route oracle runner: it replays the C# GameSession frame semantics
//! (title map event 2 -> new game -> map 1) headlessly and writes the same
//! wolf.route JSON the Godot trace test emits.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use wolf_format as wf;
use wolf_format::commonevent::CommonEventData;
use wolf_game::auto_events::AutoEvents;
use wolf_game::db::{Container, Databases};
use wolf_game::map_runtime::MapRuntime;
use wolf_vm::exec::EventId;
use wolf_vm::{
    ChoiceReply, DbOp, DbValue, Host, PictureCmd, Program, ProgramSet, SoundCmd, Vm,
};
use wolf_vm::exec::DbDirection;

use crate::{
    json_args, json_escape, json_strings, picture_action, sound_action, sound_channel,
};

struct Teleport {
    target: i32,
    map: i32,
    x: i32,
    y: i32,
    options: i32,
}

#[derive(Default)]
struct RouteHost {
    effects: Vec<String>,
    message_open: bool,
    dismissed: bool,
    pending_choices: bool,
    reply: Option<ChoiceReply>,
    teleports: VecDeque<Teleport>,
    databases: Databases,
}

impl Host for RouteHost {
    fn show_message(&mut self, _text: &str, _face: Option<i32>) {
        self.message_open = true;
        self.dismissed = false;
    }

    fn message_dismissed(&mut self) -> bool {
        self.dismissed
    }

    fn close_message(&mut self) {
        self.message_open = false;
    }

    fn show_choices(&mut self, options: &[String]) {
        self.pending_choices = true;
        self.reply = None;
        self.effects.push(format!(
            "{{\"kind\":\"choice\",\"options\":{},\"selected\":0}}",
            json_strings(options)
        ));
    }

    fn choice_made(&mut self) -> Option<ChoiceReply> {
        self.reply.take()
    }

    fn common_event_call(&mut self, target: EventId, args: &[i32], _strings: &[String]) {
        self.effects.push(format!(
            "{{\"kind\":\"call\",\"target_event_id\":{},\"args\":{}}}",
            target,
            json_args(args)
        ));
    }

    fn picture(&mut self, cmd: &PictureCmd) {
        self.effects.push(format!(
            "{{\"kind\":\"picture\",\"action\":\"{}\",\"slot\":{},\"file\":\"{}\",\"args\":{}}}",
            picture_action(cmd.action),
            cmd.number,
            json_escape(&cmd.file),
            json_args(&cmd.args)
        ));
    }

    fn sound(&mut self, cmd: &SoundCmd) {
        self.effects.push(format!(
            "{{\"kind\":\"sound\",\"action\":\"{}\",\"type\":\"{}\",\"file\":\"{}\",\"vol\":{},\"frames\":{}}}",
            sound_action(cmd.action),
            sound_channel(cmd.channel),
            json_escape(&cmd.file),
            cmd.volume,
            cmd.frames
        ));
    }

    fn database(&mut self, op: &DbOp) -> Option<DbValue> {
        let value = match &op.source {
            Some(DbValue::Num(value)) => *value,
            _ => 0,
        };
        self.effects.push(format!(
            "{{\"kind\":\"database\",\"type\":{},\"data\":{},\"item\":{},\"value\":{}}}",
            op.db_type, op.data_id, op.item_id, value
        ));
        let container = match op.container {
            wolf_vm::DbContainer::CData => Container::CData,
            wolf_vm::DbContainer::Sys => Container::Sys,
            wolf_vm::DbContainer::Data => Container::Data,
        };
        let (db_type, column) = self.databases.resolve_labels(
            container,
            op.db_type,
            op.item_id,
            op.type_label.as_deref(),
            op.field_label.as_deref(),
        );
        let row = op.data_id;
        if op.direction == DbDirection::Write {
            let databases = &mut self.databases;
            if row == wolf_game::db::ROW_COUNT_SET {
                if let Some(DbValue::Num(count)) = op.source.as_ref() {
                    databases.set_row_count(container, db_type, *count);
                }
                return None;
            }
            if column == wolf_game::db::ROW_CLEAR_COLUMN {
                databases.clear_row(container, db_type, row);
                return None;
            }
            match op.source.as_ref() {
                Some(DbValue::Num(operand)) => {
                    let current = databases
                        .engine_num(container, db_type, row, column)
                        .or_else(|| {
                            databases
                                .row_exists(container, db_type, row)
                                .then(|| databases.num(container, db_type, row, column))
                                .flatten()
                        })
                        .unwrap_or(0);
                    let value = combine_cell(op.op, current, *operand);
                    databases.set_num(container, db_type, row, column, value);
                }
                Some(DbValue::Str(text)) => {
                    databases.set_text(container, db_type, row, column, text);
                }
                None => {}
            }
            return None;
        }
        let databases = &self.databases;
        if row == wolf_game::db::ROW_COUNT_QUERY {
            let count = databases.row_count(container, db_type).unwrap_or(0);
            return Some(DbValue::Num(i32::try_from(count).unwrap_or(i32::MAX)));
        }
        let wants_text = op.destination.is_some_and(|target| {
            (wolf_vm::magic::STR_BASE..wolf_vm::magic::STR_BASE + 1_000_000).contains(&target)
        });
        let empty = || {
            if wants_text {
                DbValue::Str(String::new())
            } else {
                DbValue::Num(0)
            }
        };
        if column == wolf_game::db::FIRST_EMPTY_COLUMN {
            let row = databases.first_empty_row(container, db_type).unwrap_or(0);
            return Some(DbValue::Num(row));
        }
        if column < 0 {
            return Some(empty());
        }
        if let Some(number) = databases.engine_num(container, db_type, row, column) {
            return Some(DbValue::Num(number));
        }
        if let Some(text) = databases.engine_text(container, db_type, row, column) {
            return Some(DbValue::Str(text.to_owned()));
        }
        if databases.row_exists(container, db_type, row) {
            if let Some(number) = databases.num(container, db_type, row, column) {
                return Some(DbValue::Num(number));
            }
            if let Some(text) = databases.text(container, db_type, row, column) {
                return Some(DbValue::Str(text.to_owned()));
            }
        } else if row >= wolf_game::db::EXTENSION_ROW_BASE {
            return Some(empty());
        } else if row >= 0 && databases.has_runtime_row_count(container, db_type) {
            return Some(empty());
        }
        if databases
            .field_count(container, db_type)
            .is_some_and(|count| column >= i32::try_from(count).unwrap_or(i32::MAX))
        {
            return Some(empty());
        }
        None
    }

    fn teleport(&mut self, target: i32, map: i32, x: i32, y: i32, options: i32) {
        self.effects.push(format!(
            "{{\"kind\":\"teleport\",\"map_id\":{},\"x\":{},\"y\":{},\"direction\":{}}}",
            map, x, y, options
        ));
        self.teleports.push_back(Teleport {
            target,
            map,
            x,
            y,
            options,
        });
    }

    fn effect(&mut self, kind: i32, duration: i32, args: &[i32]) {
        self.effects.push(format!(
            "{{\"kind\":\"effect\",\"type\":{},\"duration\":{},\"args\":{}}}",
            kind,
            duration,
            json_args(args)
        ));
    }

    // GameSession's VmController answers every 123 poll with the decide code.
    fn key_code(&mut self, _mask: u32) -> i32 {
        10
    }

    fn key_code_auto(&mut self, _mask: u32, _auto: u32) -> i32 {
        10
    }

    // VmController.EngineState always returns 0, not "unknown".
    fn engine_state(&mut self, _field: i32) -> Option<i32> {
        Some(0)
    }

    fn take_trace_effects(&mut self) -> Vec<String> {
        std::mem::take(&mut self.effects)
    }
}

struct Route {
    vm: Vm,
    host: RouteHost,

    runtime: Option<MapRuntime>,
    map_id: i32,
    leader_x: i32,
    leader_y: i32,
}

impl Route {
    fn new(vm: Vm, databases: Databases) -> Self {
        Self {
            vm,
            host: RouteHost {
                databases,
                ..RouteHost::default()
            },
            runtime: None,
            map_id: -1,
            leader_x: 0,
            leader_y: 0,
        }
    }

    fn stop_all(&mut self) {
        let ids: Vec<_> = self.vm.instances().iter().map(|inst| inst.id).collect();
        for id in ids {
            self.vm.stop_instance(id);
        }
    }

    fn start_event(&mut self, map: &wf::mps::Map, event_id: u32) -> bool {
        let Some(event) = map.events.iter().find(|event| event.id == event_id) else {
            return false;
        };
        let Some(page) = event.pages.first() else {
            return false;
        };
        let program = Program::from_slots(event.id, &event.name, page.commands.clone());
        self.vm.start_program(Arc::new(program)).is_some()
    }

    fn apply_teleports(&mut self, root: &Path) {
        while let Some(request) = self.host.teleports.pop_front() {
            if request.target != -1 && request.target != 0 {
                continue;
            }
            if request.map == 3 {
                self.start_new_game(root);
                continue;
            }
            if self.map_id == request.map
                && self.leader_x == request.x
                && self.leader_y == request.y
            {
                continue;
            }
            let relative = self
                .host
                .databases
                .map_path(request.map)
                .map(str::to_owned);
            let Some(relative) = relative else {
                return;
            };
            self.leader_x = request.x;
            self.leader_y = request.y;
            self.enter_map(root, &relative, request.map);
        }
    }

    fn start_new_game(&mut self, root: &Path) {
        self.stop_all();
        self.runtime = None;
        let Some(map) = load_map(root, "MapData/Map003.mps") else {
            return;
        };
        self.start_event(&map, 1);
    }

    fn enter_map(&mut self, root: &Path, relative: &str, map_id: i32) {
        self.stop_all();
        self.runtime = None;
        // A 130 teleport map id is a SysDatabase type 0 row (column 0), exactly as
        // C# GameSession.EnterMap resolves it (D-39). Row 1 is SampleMapA and row 2
        // is SampleMapA_1; the converter artifact id is only the trace identity, so
        // map ids stay comparable with programs.json output.
        let Some(map) = load_map(root, relative) else {
            return;
        };
        // GameSession.EnterMap clamps a teleport target that lands outside the
        // destination map; the shipped new-game transfer asks for (62, 38) on a
        // 64x36 map, so the leader actually appears at (62, 35).
        let width = i32::try_from(map.width).unwrap_or(0);
        let height = i32::try_from(map.height).unwrap_or(0);
        self.leader_x = self.leader_x.clamp(0, width.saturating_sub(1).max(0));
        self.leader_y = self.leader_y.clamp(0, height.saturating_sub(1).max(0));
        let mut runtime = MapRuntime::collect(&map);
        runtime.place_leader(self.leader_x, self.leader_y, None);
        self.runtime = Some(runtime);
        self.map_id = artifact_id_for_file(root, &relative).unwrap_or(map_id);
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.start_pending(&mut self.vm);
        }
    }
}

fn artifact_id_for_file(root: &Path, relative: &str) -> Option<i32> {
    // Mirror converter/export/programs.py::_map_id: a file stem that contains
    // digits takes that number, otherwise the sorted index plus 1000.
    let target = Path::new(relative).file_name()?.to_string_lossy();
    let map_dir = root.join("MapData");
    let mut names: Vec<String> = std::fs::read_dir(map_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.to_ascii_lowercase().ends_with(".mps"))
        .collect();
    names.sort();
    for (index, name) in names.iter().enumerate() {
        if name != target.as_ref() {
            continue;
        }
        let stem = name.rsplit_once('.').map_or(name.as_str(), |(stem, _)| stem);
        let digits: String = stem.chars().filter(char::is_ascii_digit).collect();
        return Some(if digits.is_empty() {
            1000 + index as i32
        } else {
            digits.parse::<i32>().ok()?
        });
    }
    None
}

fn load_map(root: &Path, relative: &str) -> Option<wf::mps::Map> {
    let cleaned = relative.replace(std::path::MAIN_SEPARATOR, "/");
    let path: PathBuf = if cleaned.contains('/') || cleaned.ends_with(".mps") {
        PathBuf::from(cleaned)
    } else {
        PathBuf::from(format!("MapData/{cleaned}.mps"))
    };
    let bytes = std::fs::read(root.join(path)).ok()?;
    wf::mps::Map::parse(&bytes).ok()
}

fn load_title(root: &Path) -> Option<wf::mps::Map> {
    load_map(root, "MapData/TitleMap.mps")
}

fn vars_json(values: &[(i32, i32)]) -> String {
    values
        .iter()
        .map(|(address, value)| format!("\"{address}\":{value}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn combine_cell(op: u8, current: i32, operand: i32) -> i32 {
    match op {
        1 => current.wrapping_add(operand),
        2 => current.wrapping_sub(operand),
        3 => current.wrapping_mul(operand),
        4 if operand != 0 => current / operand,
        5 if operand != 0 => current % operand,
        6 => current.max(operand),
        7 => current.min(operand),
        _ => operand,
    }
}

fn str_vars_json(values: &[(i32, String)]) -> String {
    values
        .iter()
        .map(|(address, value)| format!("\"{address}\":\"{}\"", json_escape(value)))
        .collect::<Vec<_>>()
        .join(",")
}

pub fn run_route(root: &Path, frames: usize) -> ExitCode {
    let basic = root.join("BasicData");
    let events = match std::fs::read(basic.join("CommonEvent.dat")) {
        Ok(bytes) => match CommonEventData::parse(&bytes) {
            Ok(events) => events,
            Err(error) => {
                eprintln!("  ! CommonEvent.dat: {error}");
                return ExitCode::FAILURE;
            }
        },
        Err(_) => {
            eprintln!("  ! CommonEvent.dat not found under {}", basic.display());
            return ExitCode::FAILURE;
        }
    };
    let programs = ProgramSet::from_common_events(&events);
    let databases = Databases::load(&basic, (1024, 576));

    let mut vm = Vm::new(programs);
    vm.step_trace = Some(Vec::new());
    let mut route = Route::new(vm, databases);
    let mut auto_events = AutoEvents::from_common_events(&events);

    let Some(title) = load_title(root) else {
        eprintln!("  ! cannot load TitleMap.mps under {}", root.display());
        return ExitCode::FAILURE;
    };
    if !route.start_event(&title, 2) {
        eprintln!("  ! TitleMap has no event 2");
        return ExitCode::FAILURE;
    }

    let mut map_by_frame: Vec<i32> = vec![-1; frames + 2];
    for frame in 1..=frames {
        if route.host.message_open {
            route.host.message_open = false;
            route.host.dismissed = true;
        }
        if route.host.pending_choices {
            route.host.pending_choices = false;
            route.host.reply = Some(ChoiceReply::Index(0));
        }
        map_by_frame[frame] = route.map_id;
        // GameSession writes the leader/facing/action-key state here; this route
        // never moves the player and never presses the action key. Auto events are
        // edge-synced before the VM step, exactly like Game::frame.
        auto_events.sync(&mut route.vm);
        route.vm.step_frame(&mut route.host);
        if let Some(runtime) = route.runtime.as_mut() {
            runtime.start_pending(&mut route.vm);
        }
        route.apply_teleports(root);
    }

    let entries = route.vm.step_trace.as_deref().unwrap_or(&[]);
    let mut steps = String::new();
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            steps.push(',');
        }
        let map_id = map_by_frame
            .get(entry.frame as usize)
            .copied()
            .unwrap_or(route.map_id);
        let effects = entry
            .effects
            .iter()
            .map(|effect| effect.as_str())
            .collect::<Vec<_>>()
            .join(",");
        steps.push_str(&format!(
            "{{\"step\":{},\"frame\":{},\"map_id\":{},\"event_id\":{},\"pc\":{},\"op\":{},\"vars\":{{{}}},\"str_vars\":{{{}}},\"effects\":[{}]}}",
            index + 1,
            entry.frame,
            map_id,
            entry.event,
            entry.pc,
            entry.op,
            vars_json(&entry.vars),
            str_vars_json(&entry.str_vars),
            effects
        ));
    }

    let document = format!(
        "{{\"schema\":\"wolf.route\",\"schema_version\":1,\"route\":\"title_newgame_map1005\",\"frames\":{},\"total_steps\":{},\"steps\":[{}]}}",
        frames,
        entries.len(),
        steps
    );
    let out = Path::new("reports/rust_route_trace.json");
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(error) = std::fs::write(out, document) {
        eprintln!("  ! cannot write {}: {error}", out.display());
        return ExitCode::FAILURE;
    }
    println!(
        "  route     {} frame(s), {} step(s), final map {} at {},{}",
        frames,
        entries.len(),
        route.map_id,
        route.leader_x,
        route.leader_y
    );
    println!("ROUTE_TRACE_OK steps={}", entries.len());
    ExitCode::SUCCESS
}
