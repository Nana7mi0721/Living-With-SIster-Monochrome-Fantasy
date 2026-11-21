use std::collections::BTreeMap;
use std::path::PathBuf;
use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_game::db::Container;
use wolf_game::Game;

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        return p;
    }
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop();
    b.pop();
    b.join(p)
}
fn words(params: &[u8]) -> Vec<i32> {
    params
        .chunks_exact(4)
        .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}
#[test]
fn zz_debug_calls() {
    let bytes = std::fs::read(data_root().join("BasicData/CommonEvent.dat")).unwrap();
    let d = CommonEventData::parse(&bytes).unwrap();
    let mut by_title: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for id in 0u32..1200 {
        if let Some(ev) = d.event(id) {
            by_title.entry(ev.title.clone()).or_default().push(id);
        }
    }
    let interesting = [
        157u32, 158, 159, 161, 162, 163, 164, 167, 192, 193, 188, 239, 244,
    ];
    let mut c: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for id in 0u32..1200 {
        if let Some(ev) = d.event(id) {
            for (pc, slot) in ev.commands.iter().enumerate() {
                if let Slot::Command(cmd) = slot {
                    if cmd.opcode == 300 && !cmd.strings.is_empty() {
                        let name = &cmd.strings[0];
                        if let Some(ids) = by_title.get(name) {
                            for t in ids {
                                if interesting.contains(t) {
                                    c.entry(*t).or_default().push(format!(
                                        "300 from CE{} pc{} by-name {:?} args={:?}",
                                        id,
                                        pc,
                                        name,
                                        words(&cmd.params)
                                    ));
                                }
                            }
                        }
                    }
                    if cmd.opcode == 210 {
                        let w = words(&cmd.params);
                        if let Some(t) = w.first().and_then(|x| x.checked_sub(500000)) {
                            let t = t as u32;
                            if interesting.contains(&t) {
                                c.entry(t)
                                    .or_default()
                                    .push(format!("210 from CE{} pc{} args={:?}", id, pc, w));
                            }
                        }
                    }
                }
            }
        }
    }
    for t in interesting {
        println!(
            "=== target CE{} {:?}",
            t,
            d.event(t).map(|e| e.title.clone())
        );
        for line in c.get(&t).cloned().unwrap_or_default() {
            println!("  {}", line);
        }
    }
    println!("=== full args CE157 pc35..75");
    if let Some(ev) = d.event(157) {
        for pc in 35..76 {
            if let Some(Slot::Command(cmd)) = ev.commands.get(pc) {
                println!(
                    "157 pc{} op{} args={:?} strs={:?}",
                    pc,
                    cmd.opcode,
                    words(&cmd.params),
                    cmd.strings
                );
            }
        }
    }
    println!("=== full args CE159 pc230..270");
    if let Some(ev) = d.event(159) {
        for pc in 230..271 {
            if let Some(Slot::Command(cmd)) = ev.commands.get(pc) {
                println!(
                    "159 pc{} op{} args={:?} strs={:?}",
                    pc,
                    cmd.opcode,
                    words(&cmd.params),
                    cmd.strings
                );
            }
        }
    }
    println!("=== CE157 strings 0..100");
    if let Some(ev) = d.event(157) {
        for pc in 0..100 {
            if let Some(Slot::Command(cmd)) = ev.commands.get(pc) {
                if !cmd.strings.is_empty() {
                    println!("157 pc{} strs={:?}", pc, cmd.strings);
                }
            }
        }
    }
    println!("=== CE158 strings 0..140");
    if let Some(ev) = d.event(158) {
        for pc in 0..140 {
            if let Some(Slot::Command(cmd)) = ev.commands.get(pc) {
                if !cmd.strings.is_empty() {
                    println!("158 pc{} strs={:?}", pc, cmd.strings);
                }
            }
        }
    }
    println!("=== CE159 strings 100..260");
    if let Some(ev) = d.event(159) {
        for pc in 100..260 {
            if let Some(Slot::Command(cmd)) = ev.commands.get(pc) {
                if !cmd.strings.is_empty() {
                    println!("159 pc{} strs={:?}", pc, cmd.strings);
                }
            }
        }
    }
    println!("=== DATA type6 rows 0..10 all cols");
    {
        let g = Game::new(data_root()).expect("Game::new");
        for row in 0..11 {
            let mut cols: Vec<String> = Vec::new();
            for col in 0..10 {
                let txt = g
                    .databases
                    .text(Container::Data, 6, row, col)
                    .unwrap_or("")
                    .to_string();
                let num = g
                    .databases
                    .num(Container::Data, 6, row, col)
                    .unwrap_or(-999999);
                cols.push(format!("c{}={:?}/{}", col, txt, num));
            }
            println!("D6 r{} {}", row, cols.join(" | "));
        }
        println!("=== CDATA type19 rows 0..10 all cols");
        for row in 0..11 {
            let mut cols: Vec<String> = Vec::new();
            for col in 0..8 {
                let txt = g
                    .databases
                    .text(Container::CData, 19, row, col)
                    .unwrap_or("")
                    .to_string();
                let num = g
                    .databases
                    .num(Container::CData, 19, row, col)
                    .unwrap_or(-999999);
                cols.push(format!("c{}={:?}/{}", col, txt, num));
            }
            println!("C19 r{} {}", row, cols.join(" | "));
        }
        println!("=== DATA type20 row73");
        for col in 0..8 {
            println!(
                "D20 r73 c{} txt={:?} num={:?}",
                col,
                g.databases.text(Container::Data, 20, 73, col),
                g.databases.num(Container::Data, 20, 73, col)
            );
        }
    }
    println!("=== DATA type0 skill rows 0,1,2,25,26,28,29,30 selected cols");
    {
        let g = Game::new(data_root()).expect("Game::new");
        for row in [0i32, 1, 2, 25, 26, 28, 29, 30] {
            let mut cols: Vec<String> = Vec::new();
            for col in [0, 1, 2, 4, 5, 6, 9, 10, 16, 21, 23, 35] {
                let txt = g
                    .databases
                    .text(Container::Data, 0, row, col)
                    .unwrap_or("")
                    .to_string();
                let num = g
                    .databases
                    .num(Container::Data, 0, row, col)
                    .unwrap_or(-999999);
                cols.push(format!("c{}={:?}/{}", col, txt, num));
            }
            println!("D0 r{} {}", row, cols.join(" | "));
        }
    }
    println!("=== CE192 110..215");
    if let Some(ev) = d.event(192) {
        for pc in 110..216 {
            if let Some(Slot::Command(cmd)) = ev.commands.get(pc) {
                println!(
                    "192 pc{} op{} args={:?} strs={:?}",
                    pc,
                    cmd.opcode,
                    words(&cmd.params),
                    cmd.strings
                );
            }
        }
    }
    println!("=== CE192 strings 0..215");
    if let Some(ev) = d.event(192) {
        for pc in 0..216 {
            if let Some(Slot::Command(cmd)) = ev.commands.get(pc) {
                if !cmd.strings.is_empty() {
                    println!("192 pc{} strs={:?}", pc, cmd.strings);
                }
            }
        }
    }
    println!("=== CE159 0..130");
    if let Some(ev) = d.event(159) {
        for pc in 0..131 {
            if let Some(Slot::Command(cmd)) = ev.commands.get(pc) {
                println!(
                    "159 pc{} op{} args={:?} strs={:?}",
                    pc,
                    cmd.opcode,
                    words(&cmd.params),
                    cmd.strings
                );
            }
        }
    }
    println!("=== CE157 75..180");
    if let Some(ev) = d.event(157) {
        for pc in 75..181 {
            if let Some(Slot::Command(cmd)) = ev.commands.get(pc) {
                println!(
                    "157 pc{} op{} args={:?} strs={:?}",
                    pc,
                    cmd.opcode,
                    words(&cmd.params),
                    cmd.strings
                );
            }
        }
    }
    println!("=== CE65 0..180");
    if let Some(ev) = d.event(65) {
        for pc in 0..181 {
            if let Some(Slot::Command(cmd)) = ev.commands.get(pc) {
                println!(
                    "65 pc{} op{} args={:?} strs={:?}",
                    pc,
                    cmd.opcode,
                    words(&cmd.params),
                    cmd.strings
                );
            }
        }
    }
    println!("=== all op300 names containing target-ish words");
    for id in 0u32..1200 {
        if let Some(ev) = d.event(id) {
            for (pc, slot) in ev.commands.iter().enumerate() {
                if let Slot::Command(cmd) = slot {
                    if cmd.opcode == 300 && !cmd.strings.is_empty() {
                        let n = &cmd.strings[0];
                        if n.contains("対象選択")
                            || n.contains("選択実行")
                            || n.contains("行動対象")
                            || n.contains("コマンド登録")
                            || n.contains("コマンド処理")
                            || n.contains("技能選択")
                            || n.contains("アイテム選択")
                        {
                            println!("CE{} pc{} {:?} args={:?}", id, pc, n, words(&cmd.params));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn zz_trace_attack_route() {
    let mut g = Game::new(data_root()).expect("Game::new");
    g.auto_advance = true;
    g.auto_choice = true;
    g.auto_choice_delay = 1;
    let written = g.seed_battle_databases();
    println!("seed written={written}");
    let _ = g.start_event(188);
    let mut seen: std::collections::BTreeMap<String, (usize, u32)> = Default::default();
    for f in 0..800 {
        g.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = g.frame();
        for inst in g.vm.instances() {
            let ev = format!("{:?}", inst.event);
            if ev == "157" {
                println!(
                    "f{f} CE157 pc{} s1600000={} s1={} s2={} s3={} s19={} s20={} s22={} s23={}",
                    inst.pc,
                    inst.self_vars.get(1600),
                    inst.self_vars.get(1601),
                    inst.self_vars.get(1602),
                    inst.self_vars.get(1603),
                    inst.self_vars.get(1619),
                    inst.self_vars.get(1620),
                    inst.self_vars.get(1622),
                    inst.self_vars.get(1623)
                );
            }
            if [
                "157", "158", "159", "160", "161", "162", "163", "164", "167", "188", "191", "192",
                "193", "244",
            ]
            .contains(&ev.as_str())
            {
                let e = seen.entry(ev.clone()).or_insert((0, f));
                if inst.pc > e.0 {
                    e.0 = inst.pc;
                    e.1 = f;
                }
                if inst.pc == 0 {
                    println!(
                        "f{f} CE{ev} pc0 start selfs 1600000..1600030={:?}",
                        (0..31)
                            .map(|i| inst.self_vars.get(1600000 + i))
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
    }
    for (ev, (pc, f)) in &seen {
        println!("TRACE CE{} maxpc={} at_frame={}", ev, pc, f);
    }
    println!(
        "started_events={:?}",
        g.vm.started_events
            .iter()
            .filter(|e| **e >= 150 && **e <= 170
                || **e == 188
                || **e == 191
                || **e == 192
                || **e == 193
                || **e == 244)
            .collect::<Vec<_>>()
    );
    println!("started_total={}", g.vm.started_event_total);
    println!(
        "vars 340={} 349={} 350={} 352={} 343={}",
        g.vm.vars.get(340),
        g.vm.vars.get(349),
        g.vm.vars.get(350),
        g.vm.vars.get(352),
        g.vm.vars.get(343)
    );
    println!(
        "choices={:?}",
        g.choices.as_ref().map(|c| c.options.clone())
    );
    println!("warnings={}", g.stats.warnings);
}
