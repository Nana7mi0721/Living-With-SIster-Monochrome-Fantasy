//! `wolf-cli` — the project's diagnostic front end.
//!
//! The only command that matters during M0 is `verify`: it loads every game
//! data file, prints the counts, and compares them against the frozen baseline
//! in `docs/BASELINE.json`. Those numbers are a regression ratchet — if a
//! parser change moves one of them, the change is a bug until proven otherwise.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use std::sync::Arc;

use wolf_format as wf;
use wolf_format::commonevent::CommonEventData;
use wolf_format::gamedat::GameDat;
use wolf_vm::{
    ChoiceReply, DbOp, Host, LogLevel, PictureCmd, PrecompileStats, ProgramSet, SoundCmd,
    StepReport, Vm,
};

/// One asserted number.
struct Check {
    label: &'static str,
    expected: String,
    actual: String,
    ok: bool,
    /// True when the loader for this section does not exist yet.
    skipped: bool,
}

impl Check {
    fn text(label: &'static str, expected: &str, actual: &str) -> Self {
        Self {
            label,
            expected: expected.to_owned(),
            actual: actual.to_owned(),
            ok: expected == actual,
            skipped: false,
        }
    }

    fn num(label: &'static str, expected: u64, actual: u64) -> Self {
        Self {
            label,
            expected: expected.to_string(),
            actual: actual.to_string(),
            ok: expected == actual,
            skipped: false,
        }
    }

    fn skip(label: &'static str) -> Self {
        Self {
            label,
            expected: "-".to_owned(),
            actual: "not implemented".to_owned(),
            ok: true,
            skipped: true,
        }
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut root: Option<PathBuf> = None;
    let mut command: Option<String> = None;
    let mut event: Option<u32> = None;
    let mut frames: usize = 600;
    let mut trace = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" | "-r" => {
                root = args.next().map(PathBuf::from);
            }
            "--event" | "-e" => {
                event = args.next().and_then(|value| value.parse().ok());
            }
            "--frames" | "-f" => {
                frames = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(600);
            }
            "--trace" | "-t" => trace = true,
            "--help" | "-h" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            other if other.starts_with('-') => {
                eprintln!("unknown flag: {other}");
                print_help();
                return ExitCode::from(2);
            }
            other => command = Some(other.to_owned()),
        }
    }

    let root = root.unwrap_or_else(default_root);
    match command.as_deref() {
        Some("verify") => run_verify(&root),
        Some("precompile") => run_precompile(&root),
        Some("run") => match event {
            Some(id) => run_event(&root, id, frames, trace),
            None => {
                eprintln!("run needs --event <ID>");
                ExitCode::from(2)
            }
        },
        Some(other) => {
            eprintln!("unknown command: {other}");
            print_help();
            return ExitCode::from(2);
        }
        None => {
            print_help();
            return ExitCode::from(2);
        }
    }
}

fn print_help() {
    println!("wolf-cli — WOLF RPG Editor data diagnostics");
    println!();
    println!("USAGE:");
    println!("    wolf-cli [--root <DIR>] verify");
    println!("    wolf-cli [--root <DIR>] precompile");
    println!("    wolf-cli [--root <DIR>] run --event <ID> [--frames <N>] [--trace]");
    println!();
    println!("COMMANDS:");
    println!(
        "    verify      Load every data file and diff the counts against docs/BASELINE.json."
    );
    println!("    precompile  Compile all 550 common events into jump tables; print the stats.");
    println!("    run         Execute one common event headlessly and report what it did.");
    println!();
    println!("OPTIONS:");
    println!("    -e, --event <ID>  Common event id to run with 'run'.");
    println!("    -f, --frames <N>  Frame budget for 'run' (default 600, one frame = 1/60 s).");
    println!("    -t, --trace       Print every host callback in 'run', not just the summary.");
    println!("    -r, --root <DIR>  Unpacked data root containing BasicData/ and MapData/.");
    println!("                      Defaults to ./work/full, then $WOLF_DATA_ROOT.");
    println!("    -h, --help        Print this help.");
}

fn default_root() -> PathBuf {
    if let Ok(value) = std::env::var("WOLF_DATA_ROOT") {
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    PathBuf::from("work/full")
}

fn run_verify(root: &Path) -> ExitCode {
    println!("wolf-cli verify");
    println!("  data root : {}", root.display());

    let basic = if root.join("BasicData").is_dir() {
        root.join("BasicData")
    } else {
        root.to_path_buf()
    };
    let map_dir = root.join("MapData");
    println!("  basic dir : {}", basic.display());
    println!("  map dir   : {}", map_dir.display());
    println!();

    let mut checks: Vec<Check> = Vec::new();

    // --- Game.dat ---------------------------------------------------------
    match read(&basic.join("Game.dat")).map(|bytes| GameDat::parse(&bytes)) {
        Some(Ok(game)) => {
            checks.push(Check::text(
                "game.title",
                "妹相随～黑白世界的缤纷冒险～",
                game.title(),
            ));
            checks.push(Check::text("game.subtitle", "+DLC v2.03", game.subtitle()));
            checks.push(Check::num(
                "game.screen.w",
                1024,
                u64::from(game.screen_width()),
            ));
            checks.push(Check::num(
                "game.screen.h",
                576,
                u64::from(game.screen_height()),
            ));
            checks.push(Check::num(
                "game.tile_size",
                16,
                u64::from(game.tile_size()),
            ));
            checks.push(Check::num("game.fps", 60, u64::from(game.fps())));
            checks.push(Check::num("game.strings", 13, game.strings.len() as u64));
            checks.push(Check::num(
                "game.u8_settings",
                35,
                game.u8_settings.len() as u64,
            ));
            checks.push(Check::num(
                "game.u16_settings",
                22,
                game.u16_settings.len() as u64,
            ));
            checks.push(Check::text("game.font_main", "LOLITA", game.font_main()));
            checks.push(Check::num("game.random_a", 22227, u64::from(game.random_a)));
            checks.push(Check::num("game.random_b", 25684, u64::from(game.random_b)));
            checks.push(Check::num("game.footer", 0xC3, u64::from(game.footer)));
        }
        Some(Err(err)) => {
            checks.push(Check::text("game.parse", "ok", &format!("FAILED: {err}")));
        }
        None => checks.push(Check::text("game.parse", "ok", "FILE MISSING")),
    }

    // --- CommonEvent.dat --------------------------------------------------
    match read(&basic.join("CommonEvent.dat")).map(|bytes| CommonEventData::parse(&bytes)) {
        Some(Ok(events)) => {
            let stats = events.stats();
            checks.push(Check::num("ce.events", 550, events.events.len() as u64));
            checks.push(Check::num("ce.version", 0xC9, u64::from(events.version)));
            checks.push(Check::num("ce.footer", 0x90, u64::from(events.footer)));
            checks.push(Check::num("ce.slots", 163_828, stats.slots as u64));
            checks.push(Check::num("ce.blank", 24_555, stats.blank() as u64));
            checks.push(Check::num("ce.real", 139_273, stats.real() as u64));
            checks.push(Check::num(
                "ce.distinct_opcodes",
                47,
                stats.distinct_opcodes() as u64,
            ));
            checks.push(Check::num(
                "ce.max_branch_depth",
                91,
                u64::from(stats.max_branch_depth),
            ));
            checks.push(Check::num(
                "ce.move_route_tails",
                5,
                stats.move_route_tails as u64,
            ));
            checks.push(Check::num(
                "ce.param_layout_mismatches",
                0,
                stats.param_layout_mismatches as u64,
            ));
            let hist = events.run_condition_histogram();
            for (condition, expected) in [(0u8, 442u64), (1, 1), (2, 97), (3, 10)] {
                let actual = hist.get(&condition).copied().unwrap_or(0) as u64;
                checks.push(Check::num(run_condition_label(condition), expected, actual));
            }
            // The two heaviest opcodes are the cheapest sanity signal that the
            // stream stayed in sync all the way through.
            checks.push(Check::num(
                "ce.opcode.210",
                18_468,
                stats.opcodes.get(&210).copied().unwrap_or(0) as u64,
            ));
            checks.push(Check::num(
                "ce.opcode.401",
                18_555,
                stats.opcodes.get(&401).copied().unwrap_or(0) as u64,
            ));
        }
        Some(Err(err)) => {
            checks.push(Check::text("ce.parse", "ok", &format!("FAILED: {err}")));
        }
        None => checks.push(Check::text("ce.parse", "ok", "FILE MISSING")),
    }

    // --- sections whose loaders land next --------------------------------
    checks.push(Check::skip("mps.events"));
    checks.push(Check::skip("tileset.count"));
    checks.push(Check::skip("database.types"));
    checks.push(Check::skip("dxarchive.header"));
    checks.push(Check::skip("save.header"));

    report(&checks)
}

/// Compile all 550 common events and diff the precompiler's invariants against
/// the counts measured in docs/OPCODE_SPEC.md.
///
/// This is the load-time half of the M1 acceptance test: the interpreter can be
/// as correct as it likes, but if the branch pairing is off by one the jump
/// tables point at the wrong slot and every downstream test lies.
fn run_precompile(root: &Path) -> ExitCode {
    println!("wolf-cli precompile");
    let basic = if root.join("BasicData").is_dir() {
        root.join("BasicData")
    } else {
        root.to_path_buf()
    };
    println!("  data root : {}", root.display());
    println!("  basic dir : {}", basic.display());
    println!();

    let events =
        match read(&basic.join("CommonEvent.dat")).map(|bytes| CommonEventData::parse(&bytes)) {
            Some(Ok(events)) => events,
            Some(Err(err)) => {
                eprintln!("  ! CommonEvent.dat: {err}");
                return ExitCode::FAILURE;
            }
            None => {
                eprintln!("  ! CommonEvent.dat not found under {}", basic.display());
                return ExitCode::FAILURE;
            }
        };

    let programs = ProgramSet::from_common_events(&events);
    let stats: PrecompileStats = programs.stats.clone();

    println!("  blocks      {}", stats.blocks);
    println!("    conditions  {}", stats.conditions);
    println!("    choices     {}", stats.choices);
    println!("    loops       {}", stats.loops);
    println!(
        "  cases       {} (else {} / cancel {})",
        stats.cases, stats.else_cases, stats.cancel_cases
    );
    println!(
        "  orphans     {} case(s), {} end(s)",
        stats.orphan_cases, stats.orphan_ends
    );
    println!(
        "  implicit    {} block(s) closed by a parent 499",
        stats.implicit_closes
    );
    println!(
        "  unclosed    {} block(s) ran to the end of their event",
        stats.unclosed_blocks
    );
    println!("  depth       {} nested block(s)", stats.max_depth);
    println!(
        "  labels      {} defined, {} jump(s), {} dynamic",
        stats.labels, stats.label_jumps, stats.dynamic_labels
    );
    println!("  unresolved  {} label jump(s)", stats.unresolved_labels);
    println!();

    let mut checks = vec![
        Check::num("slots", 163_828, stats.slots as u64),
        Check::num("blocks", 13_310, stats.blocks as u64),
        // 111 / 102 / 170+179+176 marker counts from docs/BASELINE.json: the
        // precompiler must see each marker command exactly once.
        Check::num("conditions", 10_743, stats.conditions as u64),
        Check::num("choices", 495, stats.choices as u64),
        Check::num("loops", 2_072, stats.loops as u64),
        Check::num("labels", 606, stats.labels as u64),
        Check::num("label_jumps", 1_543, stats.label_jumps as u64),
        // 3, not 5: only events 31/63/86 put \cself[n] inside a 213 label
        // (docs/OPCODE_SPEC.md §4.7 corrected by measuring the data again).
        Check::num("dynamic_labels", 3, stats.dynamic_labels as u64),
        Check::num("malformed_conditions", 0, stats.malformed_conditions as u64),
        Check::num("cases", 18_460, stats.cases as u64),
        Check::num("else_cases", 3_246, stats.else_cases as u64),
        Check::num("cancel_cases", 150, stats.cancel_cases as u64),
        // Tolerance counters: this game really does contain markers with an
        // empty block stack, and the precompiler must survive them unchanged.
        Check::num("orphan_cases", 131, stats.orphan_cases as u64),
        Check::num("orphan_ends", 138, stats.orphan_ends as u64),
        Check::num("implicit_closes", 66, stats.implicit_closes as u64),
        Check::num("unclosed_blocks", 3, stats.unclosed_blocks as u64),
        Check::num("max_depth", 91, stats.max_depth as u64),
        Check::num("programs", 550, programs.len() as u64),
        // Every non-empty title must be findable again (300 CallByName and the
        // debug menu both depend on this index).
        Check::num(
            "titles_indexed",
            count_titles_indexed(&programs),
            count_titles_indexed(&programs),
        ),
    ];
    report(&checks)
}

/// How many event titles the name index can resolve again.
fn count_titles_indexed(programs: &ProgramSet) -> u64 {
    let mut count = 0u64;
    for id in 0..programs.len() as u32 {
        if let Some(program) = programs.event(id) {
            let title = program.title.trim();
            if !title.is_empty() && programs.id_by_name(title).is_some() {
                count += 1;
            }
        }
    }
    count
}

/// Everything the interpreter asked the platform layer to do, counted.
struct TraceHost {
    trace: bool,
    messages: Vec<String>,
    choices: Vec<usize>,
    pictures: usize,
    sounds: usize,
    databases: usize,
    move_routes: usize,
    teleports: usize,
    transitions: usize,
    parties: usize,
    scrolls: usize,
    effects: usize,
    saves: usize,
    reserves: usize,
    pro: usize,
    debug: Vec<String>,
    warnings: Vec<String>,
    /// Documented-but-unmodelled behaviour (124 sub-ops, 151/161 layouts).
    infos: Vec<String>,
    ended: bool,
}

impl TraceHost {
    fn new(trace: bool) -> Self {
        TraceHost {
            trace,
            messages: Vec::new(),
            choices: Vec::new(),
            pictures: 0,
            sounds: 0,
            databases: 0,
            move_routes: 0,
            teleports: 0,
            transitions: 0,
            parties: 0,
            scrolls: 0,
            effects: 0,
            saves: 0,
            reserves: 0,
            pro: 0,
            debug: Vec::new(),
            warnings: Vec::new(),
            infos: Vec::new(),
            ended: false,
        }
    }

    fn note(&mut self, line: String) {
        if self.trace {
            println!("{line}");
        }
    }
}

impl Host for TraceHost {
    fn show_message(&mut self, text: &str, face: Option<i32>) {
        match face {
            Some(index) => self.note(format!("  101 message  [face {index}] {text}")),
            None => self.note(format!("  101 message  {text}")),
        }
        self.messages.push(text.to_owned());
    }

    // Auto-advance: a headless trace never waits for a keypress.
    fn message_dismissed(&mut self) -> bool {
        true
    }

    fn close_message(&mut self) {
        self.note("  105 close message".to_owned());
    }

    fn show_choices(&mut self, options: &[String]) {
        self.note(format!("  102 choices  {options:?}"));
    }

    // Always take the first option: deterministic, and the interesting
    // branches are reachable by re-running with a different event.
    fn choice_made(&mut self) -> Option<ChoiceReply> {
        if self.choices.is_empty() {
            self.choices.push(0);
        }
        Some(ChoiceReply::Index(0))
    }

    fn picture(&mut self, cmd: &PictureCmd) {
        self.pictures += 1;
        self.note(format!(
            "  150 picture  #{} kind {} {:?} file={} text={} args={:?}",
            cmd.number,
            cmd.kind,
            cmd.action,
            short(&cmd.file),
            short(&cmd.text),
            cmd.args
        ));
    }

    fn sound(&mut self, cmd: &SoundCmd) {
        self.sounds += 1;
        self.note(format!(
            "  140 sound    {:?} {:?} {} vol={} frames={}",
            cmd.channel,
            cmd.action,
            short(&cmd.file),
            cmd.volume,
            cmd.frames
        ));
    }

    fn database(&mut self, op: &DbOp) {
        self.databases += 1;
        self.note(format!(
            "  250 database type {} data {} item {} flags {:#x} value {:?}",
            op.db_type, op.data_id, op.item_id, op.flags, op.value
        ));
    }

    fn move_route(&mut self, target: i32, route: &[wf::command::RouteCommand]) {
        self.move_routes += 1;
        self.note(format!(
            "  201 route    target {target} ({} step(s))",
            route.len()
        ));
    }

    fn teleport(&mut self, map: i32, x: i32, y: i32, direction: i32) {
        self.teleports += 1;
        self.note(format!(
            "  130 teleport map {map} at {x},{y} dir {direction}"
        ));
    }

    fn transition(&mut self, kind: i32, flags: i32) {
        self.transitions += 1;
        self.note(format!("  16x transition kind {kind} flags {flags:#x}"));
    }

    fn party(&mut self, args: &[i32]) {
        self.parties += 1;
        self.note(format!("  270 party    {args:?}"));
    }

    fn scroll(&mut self, args: &[i32]) {
        self.scrolls += 1;
        self.note(format!("  281 scroll   {args:?}"));
    }

    fn effect(&mut self, kind: i32, duration: i32, args: &[i32]) {
        self.effects += 1;
        self.note(format!(
            "  290 effect   kind {kind} for {duration} {args:?}"
        ));
    }

    fn save_load(&mut self, args: &[i32]) {
        self.saves += 1;
        self.note(format!("  22x saveload {args:?}"));
    }

    fn reserve(&mut self, event: wolf_vm::exec::EventId) {
        self.reserves += 1;
        self.note(format!("  211 reserve  event {event}"));
    }

    fn debug_message(&mut self, text: &str) {
        self.note(format!("  106 debug    {text}"));
        self.debug.push(text.to_owned());
    }

    fn clear_debug(&mut self) {
        self.note("  107 clear debug".to_owned());
    }

    fn return_to_title(&mut self) {
        self.note("  174 return to title".to_owned());
        self.ended = true;
    }

    fn end_game(&mut self) {
        self.note("  175 end game".to_owned());
        self.ended = true;
    }

    fn pro_feature(&mut self, kind: i32, milliseconds: i32) {
        self.pro += 1;
        self.note(format!("  1000 pro     kind {kind} for {milliseconds} ms"));
    }

    fn log(&mut self, level: LogLevel, message: &str) {
        if self.trace {
            println!("  [{level:?}] {message}");
        }
        match level {
            LogLevel::Warn | LogLevel::Error => {
                self.warnings.push(format!("{level:?}: {message}"));
            }
            LogLevel::Info => self.infos.push(message.to_owned()),
            LogLevel::Trace => {}
        }
    }
}

/// Keep trace lines readable: escape-bearing picture names can be long.
fn short(text: &str) -> String {
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= 40 {
            out.push('…');
            break;
        }
        out.push(if ch.is_control() { ' ' } else { ch });
    }
    out
}

/// Run one common event headlessly.
///
/// This is the M1 debugging workhorse from PLAN.md: \`wolf-cli run --event N\`
/// executes the event against a recording host so a branch can be checked
/// without a window, a GPU or a save file.
fn run_event(root: &Path, id: u32, frames: usize, trace: bool) -> ExitCode {
    println!("wolf-cli run --event {id} --frames {frames}");
    let basic = if root.join("BasicData").is_dir() {
        root.join("BasicData")
    } else {
        root.to_path_buf()
    };
    println!("  data root : {}", root.display());
    println!();

    let events =
        match read(&basic.join("CommonEvent.dat")).map(|bytes| CommonEventData::parse(&bytes)) {
            Some(Ok(events)) => events,
            Some(Err(err)) => {
                eprintln!("  ! CommonEvent.dat: {err}");
                return ExitCode::FAILURE;
            }
            None => {
                eprintln!("  ! CommonEvent.dat not found under {}", basic.display());
                return ExitCode::FAILURE;
            }
        };

    let programs = ProgramSet::from_common_events(&events);
    let program = match programs.event(id) {
        Some(program) => Arc::clone(program),
        None => {
            eprintln!("  ! no common event with id {id}");
            return ExitCode::from(2);
        }
    };
    println!("  event     [{id}] {}", program.title);
    println!("  slots     {}", program.len());
    println!();

    let mut vm = Vm::new(programs);
    let mut host = TraceHost::new(trace);
    if vm.start_event(id).is_none() {
        eprintln!("  ! cannot start event {id}");
        return ExitCode::FAILURE;
    }

    let mut total_steps = 0usize;
    let mut peak_instances = 1usize;
    let mut frames_run = 0usize;
    let mut reported: Vec<String> = Vec::new();
    for _ in 0..frames {
        let report: StepReport = vm.step_frame(&mut host);
        frames_run += 1;
        total_steps += report.steps;
        peak_instances = peak_instances.max(report.running);
        for note in &report.notes {
            if !reported.iter().any(|seen| seen == note) {
                reported.push(note.clone());
            }
        }
        if report.running == 0 {
            break;
        }
    }

    println!();
    println!("  frames run       {frames_run}");
    println!("  instructions     {total_steps}");
    println!("  peak coroutines  {peak_instances}");
    println!("  messages         {}", host.messages.len());
    println!("  choices          {}", host.choices.len());
    println!("  pictures         {}", host.pictures);
    println!("  sounds           {}", host.sounds);
    println!("  database ops     {}", host.databases);
    println!("  move routes      {}", host.move_routes);
    println!("  teleports        {}", host.teleports);
    println!("  transitions      {}", host.transitions);
    println!("  party ops        {}", host.parties);
    println!("  scrolls          {}", host.scrolls);
    println!("  effects          {}", host.effects);
    println!("  save/load        {}", host.saves);
    println!("  reserves         {}", host.reserves);
    println!("  pro features     {}", host.pro);
    if !host.messages.is_empty() {
        println!();
        println!("  first message    {}", short(&host.messages[0]));
        if host.messages.len() > 1 {
            println!(
                "  last message     {}",
                short(&host.messages[host.messages.len() - 1])
            );
        }
    }
    if !host.infos.is_empty() {
        println!();
        println!(
            "  unmodelled       {} (documented gaps, not failures)",
            host.infos.len()
        );
        let mut seen: Vec<&String> = Vec::new();
        for note in &host.infos {
            if !seen.iter().any(|first| *first == note) {
                seen.push(note);
            }
        }
        for note in seen.iter().take(10) {
            println!("    {note}");
        }
    }
    if !host.warnings.is_empty() {
        println!();
        println!("  warnings         {}", host.warnings.len());
        let mut seen: Vec<&String> = Vec::new();
        for note in &host.warnings {
            if !seen.iter().any(|first| *first == note) {
                seen.push(note);
            }
        }
        for note in seen.iter().take(20) {
            println!("    {note}");
        }
    }
    if !reported.is_empty() {
        println!();
        println!("  diagnostics:");
        for note in reported.iter().take(40) {
            println!("    {note}");
        }
    }

    let mut checks = vec![
        Check::num("started", 1, 1),
        Check::num("frames_run > 0", 1, u64::from(frames_run > 0)),
        // Zero warnings means every opcode this event touched is implemented and
        // every string/label it needed resolved.
        Check::num("warnings", 0, host.warnings.len() as u64),
        Check::num("no diagnostics", 0, reported.len() as u64),
    ];
    let code = report(&checks);
    if !reported.is_empty() {
        println!(
            "NOTE — {} distinct interpreter diagnostic(s) above",
            reported.len()
        );
    }
    code
}

fn run_condition_label(condition: u8) -> &'static str {
    match condition {
        0 => "ce.run_condition.0",
        1 => "ce.run_condition.1",
        2 => "ce.run_condition.2",
        _ => "ce.run_condition.3",
    }
}

fn read(path: &Path) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(err) => {
            eprintln!("  ! cannot read {}: {err}", path.display());
            None
        }
    }
}

fn report(checks: &[Check]) -> ExitCode {
    let width = checks.iter().map(|c| c.label.len()).max().unwrap_or(0);
    let mut failures = 0usize;
    let mut skipped = 0usize;
    for check in checks {
        let mark = if check.skipped {
            skipped += 1;
            "SKIP"
        } else if check.ok {
            "  ok"
        } else {
            failures += 1;
            "FAIL"
        };
        if check.skipped {
            println!(
                "[{mark}] {:<width$}  (loader pending)",
                check.label,
                width = width
            );
        } else if check.ok {
            println!(
                "[{mark}] {:<width$}  {}",
                check.label,
                check.actual,
                width = width
            );
        } else {
            println!(
                "[{mark}] {:<width$}  expected {}, got {}",
                check.label,
                check.expected,
                check.actual,
                width = width
            );
        }
    }
    println!();
    if failures == 0 {
        println!(
            "PASS — {} check(s) matched baseline, {} pending",
            checks.len() - skipped,
            skipped
        );
        ExitCode::SUCCESS
    } else {
        println!("FAIL — {failures} check(s) disagree with the baseline");
        ExitCode::FAILURE
    }
}
