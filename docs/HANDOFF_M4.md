# M4 交接说明（音频后端 + 接线完成）

> 仓库：`E:\Study\github program\imoto life`
> 基线 HEAD：`270ab37 M4: wire the audio engine into Game and add --audio`
> 前置：M3 已收口（`12ecfb6 M3 close: row-past-count DB semantics, battle slice C unreachable proof, doc closure`）；M0–M3 全部完成，M4 后端与接线完成，M4 只差“有设备时的人耳验收”。

## 1. 一句话现状

`crates/wolf-audio` 实现完整、可选设备的音频后端（symphonia 解码 ogg/mp3/wav + cpal 输出 + 软件混音，BGM 循环点读 ogg `LOOPSTART/LOOPLENGTH`），并已接进 `Game`：`Game.audio: Option<Engine>`（默认 `None`）、`Game::enable_audio()`、`GameHost.audio` 把 opcode-140 的 `SoundCmd` 交给 `apply_sound_cmd`；CLI 新增 `--audio`（capture / capture --map / dump / play 都认）。无音频设备时引擎自动退化为静音 null 后端，永不 panic，因此 headless/CI 安全。

## 2. 怎么跑 / 怎么验

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
cargo test -q --workspace                 # 全绿（含 wolf-audio 9 项 + audio_wiring）
cargo test -p wolf-audio --offline        # 纯离线，9 passed
cargo build -q -p wolf-cli
./target/debug/wolf-cli.exe verify         # PASS 63
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

1. **人耳验收**：headless 无法确认真的有声音；在一台有声卡的机器上跑 `play --root work/all --audio --load-save …` 听 BGM/SE（自动化只能证明“引擎装上、不 panic、解码与混音单测过”）。
2. **BGM 循环点来源**：当前只用 ogg 注释；mp3 BGM 无标签 ⇒ 单次播放/循环整曲。`sys.txt` 的 `BGMリスト` 循环开始(ms) 是后备，但本作 `BGMリスト` 文件名为空、`BGM番号=-3`，故未接；若遇到无标签曲需要循环，再补 manifest 后备。
3. **audio 诊断**：`apply_sound_cmd` 对缺失文件静默忽略；可加 `GameStats.audio_missing` + 诊断，便于审计资产缺口。
4. **play 默认开声**：现在默认静音（测试/CI 友好）；交互式 `play` 可考虑默认 `--audio`。
5. **交叉淡入淡出 BGM 切换**：140 的 stop 用帧数淡出，切曲目前是 stop→play；如需交叉淡化再加。
6. 若改音频行为，刷新 `docs/BASELINE.json` 的 `m4_audio` 段并重跑 `verify`。

## 5. 环境/协作陷阱

- 音频依赖必须用这套**离线可编译**特性集：`symphonia 0.6.1 { default-features=false, features=["flac","mp3","pcm","vorbis","ogg","wav"] }` + `cpal 0.18.2`。默认/all 会拉未缓存的 mkv/adpcm/aac/rustfft。
- cpal 各平台 crate（alsa/coreaudio/objc2-*）需要一次联网构建把 Cargo.lock 与缓存落好；之后 `--offline` 可复现。
- `crates/wolf-vm/src/exec.rs` 是 **CRLF**；`git commit -F work/_commit_*.txt`；截图/临时产物写 `work/out/`；不要提交 3 个 wolf-format scratch 文件。

## 6. 里程碑位置

M0–M3 done、M4 后端+接线 done（差人耳）；**M5 存档层** 已由 `save_roundtrip` 测试覆盖（逐字节 + 引擎级 8/8）；剩余大项是 **M6 主线贯通**（标题→SampleMapA 剧情，108 并行事件无死锁）。
