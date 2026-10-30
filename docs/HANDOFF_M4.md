# M4 交接说明（音频后端 + 接线完成）

> 仓库：`E:\Study\github program\imoto life`
> 基线 HEAD：`270ab37 M4: wire the audio engine into Game and add --audio`
> 前置：M3 已收口（`12ecfb6 M3 close: row-past-count DB semantics, battle slice C unreachable proof, doc closure`）；M0–M3 全部完成，M4 后端与接线完成，M4 只差“有设备时的人耳验收”。
> 外部调研（GPT deep research 6 份 + PDF）的研判索引见 `docs/research/GPT_RESEARCH_TRIAGE.md`；与本仓库实测冲突处一律以本仓库代码/已验证文档为准。

## 1. 一句话现状

`crates/wolf-audio` 实现完整、可选设备的音频后端（symphonia 解码 ogg/mp3/wav + cpal 输出 + 软件混音，BGM 循环点读 ogg `LOOPSTART/LOOPLENGTH`），并已接进 `Game`：`Game.audio: Option<Engine>`（默认 `None`）、`Game::enable_audio()`、`GameHost.audio` 把 opcode-140 的 `SoundCmd` 交给 `apply_sound_cmd`；CLI 新增 `--audio`（capture / capture --map / dump / play 都认）。无音频设备时引擎自动退化为静音 null 后端，永不 panic，因此 headless/CI 安全。

## 2. 怎么跑 / 怎么验

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
cargo test -q --workspace                 # 全绿（含 wolf-audio 9 项 + audio_wiring）
cargo test -p wolf-audio --offline        # 纯离线，9 passed
cargo build -q -p wolf-cli
./target/debug/wolf-cli.exe verify         # PASS 70
# 带声音跑（有设备就能听到）：
./target/debug/wolf-cli.exe --root work/all --audio capture \
  --load-save work/retail_backup/Save/SaveData08.sav --frames 90 \
  --out work/out/m4_hscene_audio.png       # 89.4% 非黑，音频引擎已装且不 panic
```

## 3. 关键代码位置

| 位置 | 内容 |
| --- | --- |
| `crates/wolf-audio/src/decode.rs` | symphonia 解码；`DecodedAudio { rate, channels, frames, loop_start/loop_end, samples }`；ogg 注释 `LOOPSTART/LOOPLENGTH`（帧）→ 循环点 |
| `crates/wolf-audio/src/mixer.rs` | 软件混音：SE 并发叠加裁剪、BGM 单声道循环（按 `end-start` 回绕、保留分数相位、接缝连续）、淡出归零 |
| `crates/wolf-audio/src/engine.rs` | `Engine::new/with_asset_root/null/null_with`；`play_bgm/play_se/stop_bgm/stop_all_se/set_*_volume`；`apply_sound_cmd(&SoundCmd)`；cpal 回调线程驱动 |
| `crates/wolf-game/src/game.rs` | `pub audio: Option<wolf_audio::Engine>`（默认 None）、`Game::enable_audio()`、`GameHost.audio`、`Host::sound` 转发 |
| `crates/wolf-cli/src/main.rs` | 进程级 `AUDIO` 标志 + `--audio`；三个 Game 构造点后 `if AUDIO { game.enable_audio() }` |
| `crates/wolf-cli/src/play.rs` | 同上（`crate::AUDIO`） |
| `crates/wolf-game/tests/audio_wiring.rs` | 默认静音、录音路径不变、enable_audio 无设备不 panic |
| `docs/AUDIO.md` | 完整设计（§6 已标注接线 DONE） |

## 4. 已知边界 / 下一步（按建议优先级）

1. **人耳验收（本轮已做自动部分）**：`cargo run -p wolf-audio --example m4_probe -- work/all` 在本机实测 `is_silent=false`，BGM/SE 均真实播放；离线渲染 `work/out/m4_title_fixed.wav`（6s，peak 0.8701 / rms 0.13534）与 `work/out/m4_bedroom_fixed.wav`（40s，peak 0.6441 / rms 0.03372，8 条声音含一次 SE）供人耳复听。**本轮还修正了 opcode 140 音量字段**：`intArgs[4]` 才是音量、`[1]` 是 BGM 淡入时长 / SE 延迟；旧实现读 `[1]`，导致标题 BGM 只剩 10%、多条 `yuuyake/se_akimusi2` 被当 0 静音。修正后标题从峰值 0.087 提升到 0.870、卧室音量 40/70/200/50/75 均合理。剩余只需人耳确认音色与音量。
2. **BGM 循环点来源**：当前只用 ogg 注释；mp3 BGM 无标签 ⇒ 单次播放/循环整曲。`sys.txt` 的 `BGMリスト` 循环开始(ms) 是后备，但本作 `BGMリスト` 文件名为空、`BGM番号=-3`，故未接；若遇到无标签曲需要循环，再补 manifest 后备。
3. **audio 诊断**：`apply_sound_cmd` 对缺失文件静默忽略；可加 `GameStats.audio_missing` + 诊断，便于审计资产缺口。
4. **play 默认开声**：现在默认静音（测试/CI 友好）；交互式 `play` 可考虑默认 `--audio`。
5. **交叉淡入淡出 BGM 切换**：140 的 stop 用帧数淡出，切曲目前是 stop→play；如需交叉淡化再加。
6. 若改音频行为，刷新 `docs/BASELINE.json` 的 `m4_audio` 段并重跑 `verify`。

### 4.5 M6 起点的第一块拼图：MapTree.dat 已解码（本轮新增）

- `MapTree.dat` / `MapTreeOpenStatus.dat` 已解出并进 loader：`crates/wolf-format/src/maptree.rs`、`crates/wolf-format/tests/maptree.rs`（4 测试）、`verify` 新增 7 项（总 70 项）、`docs/FORMAT_SPEC.md` §2.6。
- 结构：416 B = 10×0 头 + version 0x90 + u32 **50 条** + 50×`{i32 parent, u32 map_id}` + footer 0xBC；`parent==-1` 为根，全表仅 map **3/31/24** 挂在 map 0 下（正是三张无瓦片钩子图），条目顺序即树显示顺序。
- **否定结论（重要）**：文件里没有 x/y，也没有起始地图字段 ⇒ “新游戏从哪开始”不是 MapTree 数据驱动。M6 起点仍需从引擎启动流程实测（BOOT=TitleMap ev2 是示例标题教程；真正的新游戏入口候选是 Map003 ev1，见 `docs/MAP_AUDIT.md` §7.5）。
- **已确认（2026-10-30，本轮实测）**：真正的新游戏起点就是 **Map003 ev1**——Map003 是 MapTree 里挂在 map 0 下的钩子图（70×170，无瓦片），`[S]` 起始标记在 (62,38)，ev1 名 `ニューゲーム処理`。链路：Map003 ev1 → `121 [2000000,1]` 切游戏态 → `130 [-1,62,38,1,1]` 传送 → **SampleMapA (62,38)** → SampleMapA ev0 `ニューゲーム処理` → 卧室 + HUD。`capture --map-run Map003 --event 1` 的诊断里能看到唯一的 `130 Teleport: MapData/SampleMapA.mps at (62, 38) target -1 options 0x1`。`130` 的原始布局是 `target, x, y, map, options`（不是 `map,x,y,direction`：target=-1 是主角，地图 id 在 arg3）。
- 标题菜单路径（boot → 标题 → 确认 → CE42 选项 0）发 `130 [-1,60,34,3,1]`，按真实布局是 **target=主角、x=60、y=34、map=3（Map003）**；Map003 ev1 再发 `130 [-1,62,38,1,1]` 到 **map=1（SampleMapA）**。早期实现把 arg0 当地图 id，于是标题路径直接落到 SampleMapA、跳过了 Map003 钩子；**本轮已修正 130 布局**。CE42 是 WOLF 自带的示例标题脚本（`オリジナルタイトル画面Ａ`，3 个示例选项 1/2/3），它把 `SystemGraphic/TitleGraphic*.png` 画在**负号画面层**（`-2`）。这些画面层属于旧地图，地图切换时必须随旧图一起丢弃；早期版本没清，标题画会残留覆盖在卧室上。**本轮已修复**：`Screen::clear_screen_layers()`（只丢 `number < 0`，保留普通编号图片）在 `Game::switch_map` 里调用，`picture.rs` 有单测。修复后 `capture --boot --frames 3200 --key-frame 800:10 --answer 0 --answer-delay 2`：92.1% 非黑、49 live pictures、**192953 指令 / 9440 图片 op / 979 DB 访问 / 39 消息 / 6 选项 / 0 warning**，诊断含两条 130：`MapData/Map003.mps at (60, 34)` 与 `MapData/SampleMapA.mps at (62, 38)`，截图 `work/out/tele130_title.png` 是干净卧室+HUD（无标题画残留）。**结论：标题菜单→新游戏这条真实玩家路径现在可用且视觉正确；Map003 ev1 是 `[S]` 钩子图路径，两条入口都干净。**

### 4.6 M6 主线纵切片：标题→新游戏→SampleMapA 剧情（本轮完成，可验收）

**入口（干净、推荐）**：`--map-run Map003 --event 1`（Map003 ev1 新游戏处理 → 130 到 SampleMapA → ev0）。

```bash
./target/debug/wolf-cli.exe --root work/all capture \
  --map-run Map003 --event 1 --frames 2400 --answer 0 --answer-delay 2 \
  --out work/out/m6_map003_newgame.png
```

实测（work/all）：92.1% 非黑，180674 指令 / 9544 图片 op / 8 声音 / 994 DB 访问（296 写，335 标签）/ 39 消息 / 5 选项 / 0 warning；peak 12 coroutine；`130 Teleport: MapData/SampleMapA.mps at (62, 38) target -1 options 0x1`。截图 `work/out/m6_map003_newgame.png` = 卧室（妹妹睡在床上）+ 完整 HUD（第1天 18:30 周一、行动力 0/60、精力 3/3、所持金 2409G、方针/公会评价/状态面板、读书/钻研/漫画/查看图标）。

**剧情起点**：`--map-run SampleMapA --event 0` 前 900 帧 `--hold-message` 停在开场声明，99.3% 非黑：
「本作纯属虚构，与现实存在的人物及团体全无关联。禁止对本作品二次发行、贩卖、公布、改编、挪用解包所得的数据等贩卖行为、及其他商用行为。」（`work/out/m6_msg.png`）。

**并行事件**：注册表 108 个后台事件 = 10 个 `ENGINE_ALWAYS`（`game.rs:60-71` [48,261,269,270,271,224,225,213,214,41]）+ 1 个 `ENGINE_AUTO_START`（241）+ 97 个 `run_condition==2`（auto_events 扣掉 SKIPPED 43 后追 96，加 GATED_CALL_ONLY [242,243] 共 98 个受跟踪）。这段剧情 4000/6000 帧里实起 17 个不同 common event（受跟踪的 5 个），**0 死锁**：指令从 134k@1800 → 331k@4000 → 510k@6000 持续增长，peak 并发 12；其余 97 个是变量门控的 UI 状态，切片本来就不会全触发（属正确行为，不是缺口）。

**新增可观测性**：`crates/wolf-vm/src/exec.rs` 的 `Vm` 增 `pub started_events: BTreeSet<EventId>` 与 `pub started_event_total: u64`（`start_event_with` 里插入），CLI 的 capture 摘要新增 `background:` 行（`print_background_report`）。

**测试**：`crates/wolf-game/tests/m6_mainline.rs::new_game_story_runs_on_the_background_scheduler_without_deadlock` 固定该切片：900 帧、进度帧 >600、指令 >50k、peak≥9、warnings 0、`screen.len()>30`、所有 `ENGINE_ALWAYS` + 241 + 38 都已启动、`started_events.len()>=13`。

**前两段可玩行动（本轮新增）**：卧室是鼠标驱动的（opcode 123 指针轮询），画面里有两个可点图标（屏幕图片 121 = `1.4.0/comicon_11.png` 读书、122 = `1.4.0/comicon_10.png` 钻研）。
- 读书：capture --map-run Map003 --event 1 --frames 3000 --answer 0 --answer-delay 2 --pointer 884,240 --press-frame 1500，右下浮出「对各种事的理解加深了。」（work/out/m6_first_action.png）。
- 钻研：同一命令把 pointer 换成 195,475，右下浮出「自主训练提升各项能力。」（work/out/m6_second_action.png）。
测试 m6_mainline.rs 固定两条：clicking_the_reading_icon_in_the_bedroom_runs_the_first_playable_action（单次读书）与 the_bedroom_actions_can_be_played_one_after_another（1500 帧到卧室 → 点读书断言「理解」→ 再点钻研断言「自主训练」，证明同一局里可连续做不同行动）。主线已从「只到开场声明」推进到真实可玩交互（点击房间图标 → 行动 → 反馈），并验证了连续多动作。

**行动后的存档整合（本轮新增）**：测试 m6_mainline.rs::a_live_played_scene_survives_a_save_and_reload 在同一个真实 Game 里跑 1500 帧 → 读书 → 钻研，然后 `Game::save_to` 写盘、`savegame::load_into` 重载，断言可见画面层（图片号/文件/纯文本/坐标/透明度/缩放/锚点/window）、800 个普通变量、playtime 全部一致。已知且可接受的格式归一化：读回后 WindowString→WindowFile、文本里的 `\f[0]` 等格式码被剥成纯文本（视觉相同，比较时按纯文本+window 归一）。这说明行动结果能随存档保存并恢复。

**跨天推进（本轮新增）**：真实就寝例程 CE329（标题「┗就寝処理」）可在真实 Game 里直接启动并验证：普通变量 4=18、5=30、160=3、6=1（18:30、第 3 天、周二）为起点，300 帧后到 08:30、第 4 天、周三（var4 18→7:45(f76)→8:00(f186)→8:30(f221)，var160 3→4，var6 1→2），0 warning、15618 指令。capture --event 329 --frames 300 --var 2000004=18 --var 2000005=30 --var 2000160=3 --var 2000006=1 出图 work/out/m6_sleep.png：91.4% 非黑、6 个音效、妹妹台词「早上好。」。测试 m6_mainline.rs::the_sleep_event_rolls_over_to_the_next_morning 固定这条。**玩家菜单入口（本轮补通）**：卧室夜间点妹妹的判断在 SampleMapA ev1「妹クリック判定（夜」page3，idx141 写普通变量 63=1；该页随后打开 ev4「メニュー処理」page4 的三选一「恶作剧 / 轻轻把被子盖好 / 今天还是睡吧」。选第三项 → 「要睡到早上吗？」「是的」→ 210 [500329,1,1] 调 CE329，时钟 18:30 第 1 天 → 08:30 第 2 天（周二），0 warning。测试 m6_mainline.rs::the_bedroom_sleep_menu_choice_rolls_over_to_a_new_day 固定整条事件链（1500 帧到卧室 → 置 63=1 等价于点妹妹该页留下的旗标 → 断言三选项 → 选「今天还是睡吧」→ 断言「是的/再想想」→ 选「是的」→ 断言第 2 天 08:30）。**边界**：headless 未复刻 ev1 的指针命中判定（124/123 鼠标轮询 + 命中区），测试在事件写 63 的位置注入该旗标；菜单事件链与跨天已真实跑通。时刻显示 CE217 本身只做分钟/小时/星期回卷，天数 +1 在 CE329 就寝処理。 **跨天后 CE251 文本（本轮修复）**：清晨进入 CE251「■昼コマンド　平日」，其底部选项按钮在 pc1487/1489/1491 画 `\f[20]\cself[5..7]`，之前全部显示成裸 `0`——因为 `wolf_vm::text::expand` 把 `\cself[n]` 当成纯数值 self 槽（slot 值 0），而事件其实刚把「学习会 / 秘密特训 / 学习会」写进 self 字符串槽 5..7。修复在 `crates/wolf-vm/src/text.rs::parse_escape`：SelfVar 先读字符串槽，空时才回退数值槽（保住了 `\f[\cself[19]]` 这类字号读取）；回归断言已加进 `m6_mainline.rs::the_bedroom_sleep_menu_choice_rolls_over_to_a_new_day`（pic 1201 不再是 `\f[20]0`，且含中文标签）。截图 work/out/sleep_after_fixed.png（93.4% 非黑，按钮显示 学习会/秘密特训/学习会）。

**HUD 字宽接真实字体（本轮新增）**：文本图片的锚点/右对齐以前用半角 0.5 em 估算，数字串会比真实 Lolita.ttf 宽约 6%，右对齐 HUD 左沿偏 3–4 px。新增 `crates/wolf-game/src/fontmetrics.rs`（`FontMetrics`，用 ttf-parser 读 `Fonts/Lolita.ttf`，按字符缓存 em advance，缺字形回退半角规则），`Game` 增加可选 `font_metrics` 与 `set_font_metrics`；`Game::text_quads` 在有字体时走 `textpic::layout_with` / `measure_with`。CLI 的 `install_font_metrics`（capture/dump/play 共用 `find_font`）把同一份 TTF 喂给布局和渲染器。实测 `capture --map-run Map003 --event 1 --frames 2400`：`pic 6200`（`\f[24]18:30`）左沿由半角估算的 80 移到 83.69（零售墨迹 85），仍 92.1% 非黑、180674 指令、0 warning；截图 work/out/map003_metrics.png。测试：`fontmetrics` 3 个单测 + `crates/wolf-game/tests/text_metrics.rs`（同一 Game 在 `set_font_metrics` 前后断言右对齐左沿 80 → 83.69）。

**这条切片之外仍是缺口（M6 未完）**：① ~~标题画面层残留~~ **已解决**（§4.5：地图切换清负号画面层；capture --boot 92.1% 与 Map003 一致，回归测试 the_title_menu_new_game_path_no_longer_leaves_title_art_over_the_room）；② ~~剧情只到开场声明 + 卧室 HUD~~ **已推进到前两段可玩行动（读书/钻研，连续可做）**（上一段）；③ 201 SetMoveRoute 无地图层、290 kind 10、CData (5,16,0) 越界读仍按既有诊断透传（均不影响本切片）。
## 5. 环境/协作陷阱

- 音频依赖必须用这套**离线可编译**特性集：`symphonia 0.6.1 { default-features=false, features=["flac","mp3","pcm","vorbis","ogg","wav"] }` + `cpal 0.18.2`。默认/all 会拉未缓存的 mkv/adpcm/aac/rustfft。
- cpal 各平台 crate（alsa/coreaudio/objc2-*）需要一次联网构建把 Cargo.lock 与缓存落好；之后 `--offline` 可复现。
- `crates/wolf-vm/src/exec.rs` 实测已是 **LF**（旧文档写 CRLF 已过时）；打补丁前先 `python` 统计一次 crlf/lf，按实际换行写回。`git commit -F work/_commit_*.txt`；截图/临时产物写 `work/out/`；不要提交 3 个 wolf-format scratch 文件。

## 6. 里程碑位置

M0–M3 done、M4 后端+接线 done（差人耳）；**M5 存档层** 已由 `save_roundtrip` 测试覆盖（逐字节 + 引擎级 8/8）；剩余大项是 **M6 主线贯通**。已完成：MapTree.dat 解码（§4.5）+ 新游戏起点确认 Map003 ev1 → SampleMapA 的链路（§4.5）。已交付 M6 纵切片（§4.6）：`capture --map-run Map003 --event 1` 干净跑到卧室 HUD + 开场声明，108 后台注册表活跃、6000 帧无死锁，有 `m6_mainline.rs` 测试固定。已可验收：标题入口（capture --boot 92.1% 干净卧室）与前两段可玩行动（读书 → 理解+、钻研 → 自主训练，可连续触发；work/out/m6_first_action.png、m6_second_action.png）都已打通，m6_mainline.rs 七个测试固定（后台调度 / 标题路径无残留 / 读书点击 / 连续读书+钻研 / 行动后存档往返 / CE329 跨天 / 卧室就寝菜单跨天）。行动后的场景已经过真实 Game 的存档→重载验证；下一步可做可玩的长日常循环：引擎侧跨天已经跑通（CE329 就寝処理，18:30 第 3 天 → 08:30 第 4 天，m6_sleep.png），玩家菜单事件链也已补通（见上段），只剩 ev1 指针命中判定未在 headless 复刻。