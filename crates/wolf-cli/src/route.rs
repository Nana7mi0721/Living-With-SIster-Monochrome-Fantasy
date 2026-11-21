//! The main-route oracle runner: it replays the C# GameSession frame semantics
//! (title map event 2 -> new game -> map 1) headlessly and writes the same
//! wolf.route JSON the Godot trace test emits.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use wolf_format as wf;
use wolf_format::commonevent::CommonEventData;
use wolf_game::auto_events::AutoEvents;
use wolf_game::db::{Container, Databases};
use wolf_game::game::{seed_battle_databases_into, BattleWiring};
use wolf_game::map_runtime::MapRuntime;
use wolf_vm::exec::DbDirection;
use wolf_vm::exec::EventId;
use wolf_vm::{ChoiceReply, DbOp, DbValue, Host, PictureCmd, Program, ProgramSet, SoundCmd, Vm};

use crate::{json_args, json_escape, json_strings, picture_action, sound_action, sound_channel};

#[allow(dead_code)]
struct Teleport {
    target: i32,
    map: i32,
    x: i32,
    y: i32,
    options: i32,
}

#[derive(Clone)]
struct PictureEntry {
    kind: u8,
    file: String,
    text: String,
    args: Vec<i32>,
    anchor: u8,
    base: (f64, f64),
}

#[derive(Default)]
struct RouteHost {
    effects: Vec<String>,
    message_open: bool,
    dismissed: bool,
    pending_choices: bool,
    reply: Option<ChoiceReply>,
    choice_options: Vec<String>,
    teleports: VecDeque<Teleport>,
    databases: Databases,
    pictures: HashMap<i32, PictureEntry>,
    pointer_x: f64,
    pointer_y: f64,
    pointer_pressed: bool,
    pointer_press_pending: bool,
    pointer_press_frame: bool,
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
        self.choice_options = options.to_vec();
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
        self.track_picture(cmd);
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
        } else if row >= wolf_game::db::EXTENSION_ROW_BASE
            || (row >= 0 && databases.has_runtime_row_count(container, db_type))
        {
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

    // VmController.EngineState returns the tri-state mouse buttons (2 = pressed
    // this frame, 1 = held, 0 = up); other fields answer 0.
    fn engine_state(&mut self, field: i32) -> Option<i32> {
        match field {
            7 => Some(if self.pointer_press_frame {
                2
            } else if self.pointer_pressed {
                1
            } else {
                0
            }),
            8 | 9 => Some(0),
            _ => Some(0),
        }
    }

    fn picture_state(&mut self, number: i32, field: i32) -> Option<i32> {
        RouteHost::picture_state(self, number, field)
    }

    fn take_trace_effects(&mut self) -> Vec<String> {
        std::mem::take(&mut self.effects)
    }
}

impl RouteHost {
    fn begin_frame_input(&mut self) {
        self.pointer_press_frame = self.pointer_press_pending;
        self.pointer_press_pending = false;
    }

    fn set_pointer(&mut self, x: f64, y: f64) {
        self.pointer_x = x;
        self.pointer_y = y;
    }

    fn set_pointer_pressed(&mut self, pressed: bool) {
        if pressed && !self.pointer_pressed {
            self.pointer_press_pending = true;
        }
        self.pointer_pressed = pressed;
    }

    fn track_picture(&mut self, cmd: &PictureCmd) {
        use wolf_vm::exec::PictureAction;
        if matches!(cmd.action, PictureAction::Erase | PictureAction::Hide) {
            self.pictures.remove(&cmd.number);
            return;
        }
        if cmd.action == PictureAction::Move {
            let Some(prev) = self.pictures.get(&cmd.number).cloned() else {
                return;
            };
            let merged = PictureEntry {
                kind: cmd.kind,
                file: if cmd.file.is_empty() {
                    prev.file
                } else {
                    cmd.file.clone()
                },
                text: if cmd.text.is_empty() {
                    prev.text
                } else {
                    cmd.text.clone()
                },
                args: if cmd.args.is_empty() {
                    prev.args
                } else {
                    cmd.args.clone()
                },
                anchor: cmd.anchor,
                base: (0.0, 0.0),
            };
            let base = picture_base(&merged);
            self.pictures
                .insert(cmd.number, PictureEntry { base, ..merged });
            return;
        }
        let mut entry = PictureEntry {
            kind: cmd.kind,
            file: cmd.file.clone(),
            text: cmd.text.clone(),
            args: cmd.args.clone(),
            anchor: cmd.anchor,
            base: (0.0, 0.0),
        };
        entry.base = picture_base(&entry);
        self.pictures.insert(cmd.number, entry);
    }

    fn picture_state(&self, number: i32, field: i32) -> Option<i32> {
        let Some(entry) = self.pictures.get(&number) else {
            return match field {
                8 => Some(-1),
                9 => Some(0),
                _ => None,
            };
        };
        match field {
            8 => {
                let rect = picture_hit_rect(number, entry);
                Some(if pointer_in_rect(rect, self.pointer_x, self.pointer_y) {
                    1
                } else {
                    0
                })
            }
            9 => Some(1),
            0 => Some(ui_unit(arg(entry.args.as_slice(), 5, 0))),
            1 => Some(ui_unit(arg(entry.args.as_slice(), 6, 0))),
            2 | 3 => Some(ui_unit(arg(entry.args.as_slice(), 7, 100))),
            4 => Some(arg(entry.args.as_slice(), 9, 0)),
            5 => Some(0),
            10 => Some(if entry.kind == 2 || entry.kind == 4 {
                1
            } else {
                -1
            }),
            _ => None,
        }
    }
}

struct Route {
    vm: Vm,
    host: RouteHost,

    runtime: Option<MapRuntime>,
    map_id: i32,
    leader_x: i32,
    leader_y: i32,
    facing: i32,
    auto_choice: bool,
    auto_events: AutoEvents,
    battle_wiring: BattleWiring,
}

impl Route {
    fn new(vm: Vm, databases: Databases, auto_events: AutoEvents) -> Self {
        Self {
            vm,
            host: RouteHost {
                databases,
                pointer_x: -1.0,
                pointer_y: -1.0,
                ..RouteHost::default()
            },
            runtime: None,
            map_id: -1,
            leader_x: 0,
            leader_y: 0,
            facing: 2,
            auto_choice: true,
            auto_events,
            battle_wiring: BattleWiring::new(),
        }
    }

    fn set_auto_choice(&mut self, enabled: bool) {
        self.auto_choice = enabled;
    }

    fn choose(&mut self, index: i32) {
        self.host.pending_choices = false;
        self.host.reply = Some(ChoiceReply::Index(index as usize));
    }

    fn has_event(&self, event: u32) -> bool {
        self.vm.instances().iter().any(|inst| inst.event == event)
    }

    fn var(&self, id: i32) -> i32 {
        self.vm.vars.get(id as usize)
    }

    fn set_var(&mut self, id: i32, value: i32) {
        self.vm.vars.set(id as usize, value);
    }

    fn picture_state(&self, number: i32, field: i32) -> Option<i32> {
        self.host.picture_state(number, field)
    }

    fn seed_battle(&mut self) -> usize {
        seed_battle_databases_into(&mut self.host.databases)
    }

    /// One frame with the exact `GameSession.AdvanceFrame` ordering: scripted message
    /// dismissal and choice auto-pick, map action-key trigger, AutoEvents, BattleWiring,
    /// VM step, map pending starts, then teleports at the frame boundary.
    fn advance(&mut self, root: &Path, action_pressed: bool) {
        self.host.begin_frame_input();
        if self.host.message_open {
            self.host.message_open = false;
            self.host.dismissed = true;
        }
        if self.host.pending_choices && self.auto_choice {
            self.host.pending_choices = false;
            self.host.reply = Some(ChoiceReply::Index(0));
        }
        if action_pressed
            && !self.host.message_open
            && !self.host.pending_choices
            && (self.vm.input.basic_all_deny & 1) == 0
        {
            let Route {
                runtime,
                vm,
                leader_x,
                leader_y,
                facing,
                ..
            } = self;
            if let Some(runtime) = runtime.as_mut() {
                let requests = runtime.try_trigger((*leader_x, *leader_y), *facing, true, false);
                for request in requests {
                    if runtime.start_trigger_page(request, vm) {
                        break;
                    }
                }
            }
        }
        let Route {
            vm,
            host,
            auto_events,
            battle_wiring,
            ..
        } = self;
        auto_events.sync(vm);
        battle_wiring.tick(vm, &host.databases);
        vm.step_frame(host);
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.start_pending(&mut self.vm);
        }
        self.apply_teleports(root);
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
            let relative = self.host.databases.map_path(request.map).map(str::to_owned);
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
        self.map_id = artifact_id_for_file(root, relative).unwrap_or(map_id);
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
        let stem = name
            .rsplit_once('.')
            .map_or(name.as_str(), |(stem, _)| stem);
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

fn arg(args: &[i32], index: usize, fallback: i32) -> i32 {
    args.get(index).copied().unwrap_or(fallback)
}

fn ui_unit(raw: i32) -> i32 {
    if raw > 100_000 {
        raw & 0xFFFF
    } else {
        raw
    }
}

fn picture_base(entry: &PictureEntry) -> (f64, f64) {
    let kind = entry.kind;
    if kind == 2 || kind == 4 {
        return (
            f64::from(arg(&entry.args, 1, 0).max(0)),
            f64::from(arg(&entry.args, 2, 0).max(0)),
        );
    }
    if kind == 3 && arg(&entry.args, 1, 0) > 2 && arg(&entry.args, 2, 0) > 2 {
        return (
            f64::from(arg(&entry.args, 1, 0)),
            f64::from(arg(&entry.args, 2, 0)),
        );
    }
    (420.0, 140.0)
}

fn picture_hit_rect(number: i32, entry: &PictureEntry) -> (f64, f64, f64, f64) {
    if !(0..12_000).contains(&number) {
        return (0.0, 0.0, 1024.0, 576.0);
    }
    let (base_w, base_h) = entry.base;
    if base_w == 0.0 || base_h == 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let scale_percent = ui_unit(arg(&entry.args, 7, 100));
    let factor = if scale_percent == 0 {
        1.0
    } else {
        f64::from(scale_percent.abs()) / 100.0
    };
    let width = base_w * factor;
    let height = base_h * factor;
    let x = f64::from(ui_unit(arg(&entry.args, 5, 0)));
    let y = f64::from(ui_unit(arg(&entry.args, 6, 0)));
    let (left, top) = match entry.anchor {
        1 => (x - width * 0.5, y - height * 0.5),
        2 => (x, y - height),
        3 => (x - width, y),
        4 => (x - width, y - height),
        5 => (x - width * 0.5, y),
        6 => (x - width * 0.5, y - height),
        _ => (x, y),
    };
    (left, top, width, height)
}

fn pointer_in_rect(rect: (f64, f64, f64, f64), x: f64, y: f64) -> bool {
    let (left, top, width, height) = rect;
    x >= left && x < left + width && y >= top && y < top + height
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
    let mut route = Route::new(vm, databases, AutoEvents::from_common_events(&events));

    let Some(title) = load_title(root) else {
        eprintln!("  ! cannot load TitleMap.mps under {}", root.display());
        return ExitCode::FAILURE;
    };
    if !route.start_event(&title, 2) {
        eprintln!("  ! TitleMap has no event 2");
        return ExitCode::FAILURE;
    }

    let mut map_by_frame: Vec<i32> = vec![-1; frames + 2];
    for frame_slot in map_by_frame.iter_mut().take(frames + 1).skip(1) {
        if route.host.message_open {
            route.host.message_open = false;
            route.host.dismissed = true;
        }
        if route.host.pending_choices {
            route.host.pending_choices = false;
            route.host.reply = Some(ChoiceReply::Index(0));
        }
        *frame_slot = route.map_id;
        // GameSession writes the leader/facing/action-key state here; this route
        // never moves the player and never presses the action key. Auto events are
        // edge-synced before the VM step, exactly like Game::frame.
        route.auto_events.sync(&mut route.vm);
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

struct FullDriver<'a> {
    route: Route,
    root: &'a Path,
    map_by_frame: Vec<i32>,
    frames: usize,
    pointer_x: f64,
    pointer_y: f64,
    mouse_down: bool,
    action_queued: bool,
    errors: Vec<String>,
}

impl<'a> FullDriver<'a> {
    fn new(route: Route, root: &'a Path) -> Self {
        Self {
            route,
            root,
            map_by_frame: vec![-1],
            frames: 0,
            pointer_x: -1.0,
            pointer_y: -1.0,
            mouse_down: false,
            action_queued: false,
            errors: Vec::new(),
        }
    }

    fn fail(&mut self, message: impl Into<String>) {
        let message = message.into();
        eprintln!("  ! {message}");
        self.errors.push(message);
    }

    fn step(&mut self) {
        self.frames += 1;
        self.map_by_frame.push(self.route.map_id);
        self.route.host.set_pointer(self.pointer_x, self.pointer_y);
        self.route.host.set_pointer_pressed(self.mouse_down);
        let action = std::mem::take(&mut self.action_queued);
        self.route.advance(self.root, action);
    }

    fn tick(&mut self, count: usize) {
        for _ in 0..count {
            self.step();
        }
    }

    fn action(&mut self) {
        self.action_queued = true;
    }

    fn mouse_move(&mut self, x: i32, y: i32) {
        self.pointer_x = f64::from(x);
        self.pointer_y = f64::from(y);
    }

    fn mouse_down(&mut self, down: bool) {
        self.mouse_down = down;
    }

    fn click_at(&mut self, x: i32, y: i32) {
        self.mouse_move(x, y);
        self.tick(10);
        for _ in 0..5 {
            self.mouse_down(true);
            self.step();
        }
        for _ in 0..5 {
            self.mouse_down(false);
            self.step();
        }
    }

    fn wait_picture(&mut self, number: i32, limit: usize) -> bool {
        for _ in 0..limit {
            if self.route.picture_state(number, 9) == Some(1) {
                return true;
            }
            self.step();
        }
        self.route.picture_state(number, 9) == Some(1)
    }

    fn wait_clock(&mut self, day: i32, hour: i32, minute: i32, limit: usize) -> bool {
        for _ in 0..limit {
            if self.route.var(160) == day
                && self.route.var(4) == hour
                && self.route.var(5) >= minute
            {
                return true;
            }
            self.step();
        }
        self.route.var(160) == day && self.route.var(4) == hour && self.route.var(5) >= minute
    }

    fn select_day_row(&mut self, day: i32) {
        if !self.wait_picture(1201, 2200) {
            self.fail(format!("day {day}: day row picture 1201 missing"));
            return;
        }
        if !self.wait_picture(1200, 400) {
            self.fail(format!("day {day}: day row picture 1200 missing"));
            return;
        }
        if self.route.var(160) != day || self.route.var(4) < 8 {
            self.fail(format!(
                "day {day}: row menu opened at day={} {}:{}",
                self.route.var(160),
                self.route.var(4),
                self.route.var(5)
            ));
        }
        let before_command = self.route.var(48);
        let row_x = self.route.picture_state(1200, 0).unwrap_or(512);
        let row_y = self.route.picture_state(1200, 1).unwrap_or(0);
        self.mouse_move(row_x, row_y);
        self.tick(12);
        for _ in 0..6 {
            self.mouse_down(true);
            self.step();
        }
        for _ in 0..5 {
            self.mouse_down(false);
            self.step();
        }
        self.mouse_move(-1, -1);
        let mut latched = false;
        for _ in 0..2600 {
            if self.route.var(48) != before_command {
                latched = true;
                break;
            }
            self.step();
        }
        if !latched {
            self.fail(format!(
                "day {day}: CE251 row click did not change command var48={}",
                self.route.var(48)
            ));
        }
    }

    fn spend_day(&mut self, day: i32) {
        let before_energy = self.route.var(14);
        self.select_day_row(day);
        if !self.errors.is_empty() {
            return;
        }
        let mut evening = false;
        for _ in 0..3200 {
            if self.route.var(4) >= 17 && !self.route.has_event(251) {
                evening = true;
                break;
            }
            self.step();
        }
        if !evening {
            self.fail(format!(
                "day {day}: CE251 daytime did not end (hour={} ev251={})",
                self.route.var(4),
                self.route.has_event(251)
            ));
        }
        if self.route.var(14) >= before_energy {
            self.fail(format!(
                "day {day}: energy did not fall ({}->{})",
                before_energy,
                self.route.var(14)
            ));
        }
        let mut settled = false;
        for _ in 0..6000 {
            if !self.route.has_event(300) && !self.route.has_event(326) && self.route.var(4) >= 18 {
                settled = true;
                break;
            }
            self.step();
        }
        if !settled {
            self.fail(format!(
                "day {day}: evening did not settle (hour={} ev300={} ev326={})",
                self.route.var(4),
                self.route.has_event(300),
                self.route.has_event(326)
            ));
        }
        if !self.wait_picture(120, 2600) {
            self.fail(format!("day {day}: sister picture 120 did not appear"));
        }
    }

    fn sleep_to_next_morning(&mut self, day: i32) {
        let expected = day + 1;
        if self.route.var(295) == 1 {
            self.route.set_var(295, 0);
        }
        let mut evening = false;
        for _ in 0..3000 {
            if self.route.var(160) == day && self.route.var(4) >= 18 {
                evening = true;
                break;
            }
            self.step();
        }
        if !evening {
            self.fail(format!(
                "sleep day {day}: evening not reached (day={} {}:{})",
                self.route.var(160),
                self.route.var(4),
                self.route.var(5)
            ));
            return;
        }
        let has_picture = self.wait_picture(120, 600);
        if day > 1 {
            let mut menu = false;
            for _ in 0..6000 {
                if self.route.has_event(245) {
                    menu = true;
                    break;
                }
                self.step();
            }
            if !menu {
                self.fail(format!("sleep day {day}: night menu CE245 did not appear"));
            }
        }
        if !has_picture && !self.wait_picture(120, 600) {
            self.fail(format!("sleep day {day}: sister picture 120 missing"));
            return;
        }
        self.route.set_auto_choice(false);
        let sx = self.route.picture_state(120, 0).unwrap_or(240);
        let sy = self.route.picture_state(120, 1).unwrap_or(340);
        self.mouse_move(sx, sy);
        self.tick(8);
        self.action();
        self.tick(40);
        let mut menu_after = false;
        for _ in 0..400 {
            if self.route.has_event(245) {
                menu_after = true;
                break;
            }
            self.step();
        }
        if !menu_after {
            self.fail(format!(
                "sleep day {day}: CE245 disappeared before sleep choice"
            ));
        }
        self.click_at(512, 450);
        let mut got_choice = false;
        for _ in 0..500 {
            if self.route.host.pending_choices {
                got_choice = true;
                break;
            }
            self.step();
        }
        let options = self.route.host.choice_options.clone();
        if !got_choice
            || options.len() != 2
            || options.first().map(String::as_str) != Some("是")
            || options.get(1).map(String::as_str) != Some("算了")
        {
            self.fail(format!(
                "sleep day {day}: choice missing or wrong [{}]",
                options.join(",")
            ));
        }
        self.route.choose(0);
        self.route.set_auto_choice(true);
        if day == 3 {
            let mut morning = false;
            for index in 0..3200 {
                if self.route.var(160) == 4 && self.route.var(4) == 8 && self.route.var(5) >= 30 {
                    morning = true;
                    break;
                }
                if index & 1 == 0 {
                    self.action();
                }
                self.step();
            }
            if !morning {
                self.fail(format!(
                    "sleep day 3: day4 08:30 not reached (day={} {}:{})",
                    self.route.var(160),
                    self.route.var(4),
                    self.route.var(5)
                ));
            }
        } else if !self.wait_clock(expected, 8, 30, 3200) {
            self.fail(format!(
                "sleep day {day}: day{expected} 08:30 not reached (day={} {}:{})",
                self.route.var(160),
                self.route.var(4),
                self.route.var(5)
            ));
        }
    }

    fn run(&mut self) {
        self.sleep_to_next_morning(1);
        if !self.errors.is_empty() {
            return;
        }
        self.spend_day(2);
        if !self.errors.is_empty() {
            return;
        }
        self.sleep_to_next_morning(2);
        if !self.errors.is_empty() {
            return;
        }
        self.spend_day(3);
        if !self.errors.is_empty() {
            return;
        }
        let seeded = self.route.seed_battle();
        if seeded < 26 {
            self.fail(format!("battle seed wrote {seeded} cells, expected >= 26"));
        }
        self.sleep_to_next_morning(3);
        if !self.errors.is_empty() {
            return;
        }
        self.spend_day(4);
        if !self.errors.is_empty() {
            return;
        }
        self.sleep_to_next_morning(4);
    }
}

pub fn run_full_route(root: &Path) -> ExitCode {
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
    let mut route = Route::new(vm, databases, AutoEvents::from_common_events(&events));

    let Some(title) = load_title(root) else {
        eprintln!("  ! cannot load TitleMap.mps under {}", root.display());
        return ExitCode::FAILURE;
    };
    if !route.start_event(&title, 2) {
        eprintln!("  ! TitleMap has no event 2");
        return ExitCode::FAILURE;
    }

    let mut driver = FullDriver::new(route, root);
    let mut bedroom = false;
    for _ in 0..1600 {
        if driver.route.map_id == 1005 {
            bedroom = true;
            break;
        }
        driver.step();
    }
    if !bedroom {
        eprintln!(
            "  ! bedroom not reached: map={} frames={}",
            driver.route.map_id, driver.frames
        );
        return ExitCode::FAILURE;
    }
    driver.route.set_var(295, 0);
    driver.route.set_var(68, 0);

    driver.run();
    let entries = driver.route.vm.step_trace.as_deref().unwrap_or(&[]);
    let mut steps = String::new();
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            steps.push(',');
        }
        let map_id = driver
            .map_by_frame
            .get(entry.frame as usize)
            .copied()
            .unwrap_or(driver.route.map_id);
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
        "{{\"schema\":\"wolf.route\",\"schema_version\":1,\"route\":\"m4_full_day_ce253_f5f9\",\"frames\":{},\"total_steps\":{},\"steps\":[{}]}}",
        driver.frames,
        entries.len(),
        steps
    );
    let out = Path::new("reports/rust_m4_route_trace.json");
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(error) = std::fs::write(out, document) {
        eprintln!("  ! cannot write {}: {error}", out.display());
        return ExitCode::FAILURE;
    }
    println!(
        "  route     {} frame(s), {} step(s), final day {} {}:{}, map {} at {},{}",
        driver.frames,
        entries.len(),
        driver.route.var(160),
        driver.route.var(4),
        driver.route.var(5),
        driver.route.map_id,
        driver.route.leader_x,
        driver.route.leader_y
    );
    println!(
        "  battle    rounds={} turns={}",
        driver.route.battle_wiring.started_rounds(),
        driver.route.battle_wiring.turn_starts()
    );
    if !driver.errors.is_empty() {
        for error in &driver.errors {
            eprintln!("  ! {error}");
        }
        return ExitCode::FAILURE;
    }
    if driver.route.var(160) != 5 || driver.route.var(4) != 8 || driver.route.var(5) < 30 {
        eprintln!(
            "  ! final clock day={} {}:{}, expected day5 08:30",
            driver.route.var(160),
            driver.route.var(4),
            driver.route.var(5)
        );
        return ExitCode::FAILURE;
    }
    println!("M4_ROUTE_OK steps={}", entries.len());
    ExitCode::SUCCESS
}
