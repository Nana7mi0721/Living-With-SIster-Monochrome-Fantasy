# TOOLING — 本机已安装工具链清单

> 生成于本次会话（m00934 用户请求：「那么你先下载对接下来工作有帮助好便利的相关工具，请尽情安装」）。
> 所有工具安装在项目内 `tools/` 下，不污染系统目录；PATH 已写入**用户级**环境变量。

## 0. 一句话

Rust 1.99.1 + MSVC 18.9 + wgpu/winit/glyphon/kira 全家桶 **已实测可编译**（`cargo check` 44s 通过），
配合 ripgrep/fd/just/jq/ffmpeg/tokei/hexyl/hyperfine 等命令行工具，
下一次会话可以直接 `cargo new` 开始写 `wolf-format`，不需要再做任何环境准备。

## 1. 快速进入环境

git-bash（**必须用 msys 路径** `/e/...`，Windows 风格 `E:/...` 在 PATH 里静默失效）：

```bash
source "/e/Study/github program/imoto life/tools/env.sh"
# [env] rustc 1.99.1 | rg 15.2.0
```

PowerShell：

```powershell
. "E:\Study\github program\imoto life\tools\env.ps1"
```

新开的终端（用户 PATH 已持久化，无需 source）：
`tools/bin` · `tools/cmake/bin` · `tools/ffmpeg/bin` · `%USERPROFILE%\.cargo\bin`

## 2. Rust 工具链

| 组件 | 版本 | 位置 |
| --- | --- | --- |
| rustc | **1.99.1** (58cf8f9c3 2026-10-13) | `C:\Users\REISEN\.cargo\bin\rustc.exe` |
| cargo | **1.99.1** (5f94df478 2026-08-27) | 同上 |
| rustup | 1.29.2 | 同上 |
| rustfmt | 1.10.0-stable | 同上 |
| cargo clippy | 0.1.99 | `cargo clippy` |
| rust-analyzer | 1.99.1 | 同上 |
| rust-src | `rustup component add rust-src` 已装 | — |
| target | `stable-x86_64-pc-windows-msvc`（默认） | — |

链接器来自 **Visual Studio Build Tools 18.9.12105.275**（已有，未额外安装）：

- MSVC toolset **14.51.36231**：`C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Tools\MSVC\14.51.36231`
- Windows SDK **10.0.26100.0**：`C:\Program Files (x86)\Windows Kits\10\bin`

### 2.1 `~/.cargo/config.toml`（已写好，改动务必保留）

```toml
[source.crates-io]
replace-with = "ustc"

[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"

[registries.ustc]
index = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"

[net]
git-fetch-with-cli = true
retry = 3

[http]
proxy = ""          # ★ 关键：不清空会走 Windows 系统代理 127.0.0.1:7890 然后失败
```

**`[http] proxy = ""` 是必须的**：本机 Windows 注册表设了系统代理
`HKCU\...\Internet Settings\ProxyServer = 127.0.0.1:7890`，cargo 通过 libcurl **会读注册表代理**，
而普通 shell `curl` **不读** —— 所以现象是「curl 全通、cargo 全挂」。清空即可。

### 2.2 已安装的 cargo 子命令

`cargo-nextest 0.9.149` · `cargo-machete 0.9.2` · `cargo-expand 1.0.127` ·
`cargo-watch 8.5.3` · `cargo-bloat 0.12.1` · `cargo-edit 0.13.14`（`cargo upgrade`/`add`/`rm`）

安装方式：`cargo install --locked <crate>`（见 §6 关于 cargo-binstall 的说明）。

## 3. 命令行工具（`tools/bin`）

| 工具 | 版本 | 用途 |
| --- | --- | --- |
| `rg` | ripgrep 15.2.0 | 搜代码/搜 JSON dump |
| `fd` | 10.5.0 | 按名找文件 |
| `bat` | 0.26.1 | 带语法高亮看文件 |
| `just` | 1.59.0 | 任务运行器（替代 make） |
| `delta` | 0.20.1 | 更好的 git diff |
| `jq` | 1.8.2 | 处理 WolfTL 的 JSON dump / `docs/BASELINE.json` |
| `hexyl` | 0.17.0 | 彩色 hexdump（看 .dat/.mps 头） |
| `tokei` | 12.1.2 | 统计 LOC（对照「~18k–25k 行 Rust」估算） |
| `hyperfine` | 1.19.0 | 基准测试（VM 每帧指令数、加载耗时） |
| `oxipng` | 9.1.5 | 无损压缩 PNG（打包资源用） |
| `ninja` | 1.13.2 | CMake 后端（编译第三方 C 依赖时用） |
| `7zr` | 26.04 | 解压 7z |
| `cmake` | **4.4.4** | `tools/cmake/bin` |
| `ffmpeg`/`ffprobe`/`ffplay` | **8.1 essentials** | `tools/ffmpeg/bin`；已实测解析 `work/alldata/SE/ok01.mp3` → mp3/44100/stereo |

## 4. Python（系统 Python 3.10.11）

已有：`Pillow 12.3.0` · `numpy 2.2.6`

本次新装：`kaitaistruct 0.11` · `construct 2.10.70` · `pytest 9.1.1` · `rich` · `orjson`

> 注意：**本机没有 Java**，所以 `kaitai-struct-compiler`（JVM 工具）用不了。
> `_wolfref/*.ksy` 只能当**规格文档**读，或手写 Python/Rust 解析器 —— 这也是原计划的做法。

## 5. 已实测的编译基线

`tools/wgpusmoke/`（保留作回归用）：
`wgpu 30.0.1` + `winit 0.30.13` + `glyphon 0.12.0` + `kira 0.12.5` + `pollster` + `glam` + `bytemuck`

```
cargo check   →  Finished `dev` profile ... in 44.13s
```

解析到的关键 crate：`wgpu-naga-bridge 30.0.1` · `cosmic-text 0.19.0` · `swash 0.2.10` ·
`harfrust 0.5.2` · `skrifa 0.40.0/0.44.0` · `symphonia 0.6.1`（mp3/ogg/flac/wav/pcm/riff）· `windows 0.62.2`

⇒ **`docs/ARCHITECTURE.md` §3/§5 的依赖表被证实可用**；kira 自带 mp3/ogg 解码，
不需要额外音频解码库，也不需要 MIDI（本游戏 0 条 MIDI 引用）。

### 5.1 GPU 真机跑通（不只是能编译）

`src/main.rs` 是一个**真实离屏渲染**冒烟测试（枚举适配器 → 请求 device → 渲染三角形到 1024×576 纹理 → submit → poll）。

```
cargo run     →  adapter count = 5
  [DiscreteGpu] NVIDIA GeForce RTX 5060 Laptop GPU | backend=Vulkan | driver=NVIDIA 610.88      | 0x10de:0x2d59
  [DiscreteGpu] NVIDIA GeForce RTX 5060 Laptop GPU | backend=Dx12   | driver=32.0.16.1088       | 0x10de:0x2d59
  [DiscreteGpu] NVIDIA GeForce RTX 5060 Laptop GPU | backend=Dx12   | driver=32.0.16.1088       | 0x10de:0x2d59
  [Cpu]         Microsoft Basic Render Driver      | backend=Dx12   | driver=10.0.26100.4484     | 0x1414:0x8c
  [Other]       NVIDIA GeForce RTX 5060 Laptop GPU/PCIe/SSE2 | backend=Gl | driver=4.6.0 NVIDIA 610.88
chosen adapter: NVIDIA GeForce RTX 5060 Laptop GPU (DiscreteGpu, Vulkan)
max_texture_dimension_2d = 32768
max_buffer_size = 4095 MiB
OFFSCREEN RENDER OK
```

⇒ **渲染管线在本机确实能跑**（Vulkan 优先，Dx12 与 GL 都可用），1024×576 就是本游戏的逻辑分辨率。

**这次编译暴露的 wgpu 30 API 陷阱**（原 `ARCHITECTURE.md` §3.2 未记录，现已补进该表）：

- `Instance::new(desc)` 收**按值**，不是引用
- `instance.enumerate_adapters(b)` 返回 **`impl Future`** —— 要 `block_on`
- `RequestAdapterOptions` 新增 `apply_limit_buckets: bool`
- `RenderPipelineDescriptor.multiview` → **`multiview_mask`**
- `PollType::Wait { submission_index, timeout }` 是 **struct variant**，不是常量
- `InstanceDescriptor` 没有 `Default`，用 `new_without_display_handle()`

`tools/smoke/`：`binrw 0.15.2` + `serde_json` —— 二进制解析层可用。

## 6. 两个「本机特有问题」（下次别再踩）

1. **`cargo-binstall` 在本机不可用**：它在 `%USERPROFILE%\.cargo\` 下建临时设置文件
   `.tmpXXXXXX` 时被拒（`I/O error: 拒绝访问。 (os error 5)`），重试无效、手建 `binstall.toml` 也无效；
   而同样目录下 bash `echo >`、Python `tempfile`/`O_EXCL` 都正常。**用 `cargo install --locked` 代替**。
2. **`sccache` 会让 MSVC 链接崩**：`rustc-wrapper = sccache` 后 build script 链接报
   `LINK : fatal error LNK1104: 无法打开文件"C:\Users\REISEN\AppData\Local\Temp\lnk{GUID}.tmp"`。
   移除后恢复正常。**本机不要用 sccache**；16 核机器裸编译已经够快（全 wgpu 栈 44s）。

同类现象还有 rustup 自拷贝被拒（`os error 5`，但手 `cp` 成功）——
本机对「进程自己创建/改名临时文件」有某种拦截，遇到 `拒绝访问 (os error 5)` 先想「换个不走临时文件的等价做法」。

## 7. 下载通道（重要）

- **GitHub 直连几乎不可用**（~2 KB/s，ripgrep zip 180s 只下了 400 KB）。
- **可用代理：`https://ghfast.top/`** —— 前缀式，形如
  `https://ghfast.top/https://github.com/<owner>/<repo>/releases/download/<tag>/<asset>`
  （实测 ninja 5.95s、sccache 9.8MB 18.6s、cmake 54MB、ffmpeg 108MB/52.7s；`gh-proxy.com` 可用但稍慢；`ghproxy.net` 很慢；`mirror.ghproxy.com` 已死）
- `releases/latest/download/<name>` 这种别名在这条通道上**常常 404**，要用 **GitHub API 查准 tag**
  （`https://api.github.com/repos/<o>/<r>/releases/latest` **直连可用**，走 ghfast 反而 403）。
- rustup 走 **USTC 镜像**（1.4 MB/s）：`mirrors.ustc.edu.cn/rust-static/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe`
- USTC/TUNA 的 github-release 镜像**没有** Kitware/CMake 和 FFmpeg，别在那上面浪费时间。

## 8. 相关文档

- [PLAN.md](PLAN.md) — 总体重写计划
- [docs/FORMAT_SPEC.md](FORMAT_SPEC.md) — 二进制格式规格
- [docs/OPCODE_SPEC.md](OPCODE_SPEC.md) — 50 条 opcode 语义
- [docs/ARCHITECTURE.md](ARCHITECTURE.md) — crate 架构与依赖表
- [docs/BASELINE.json](BASELINE.json) — 机器可校验的基线 oracle（`jq` 已实测可读：550 / 73 / 169831）
