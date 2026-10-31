# Windows 本机部署与源码分析

验证日期：2026-10-30。仓库：`kaliwell/MusicTerrain`（私有）。
初次拉取基线：`b572043483d25a6963a5d3aeb5613b699880af0d`，后续更新见 Git 提交记录；本地 `main` 跟踪 `origin/main`。

## 启动

双击仓库根目录的 `Start-MusicTerrain.cmd`，默认播放内置的 12 秒示例。
将 WAV、MP3 或 FLAC 拖入窗口即可导入自己的音乐。

也可以在“设置”开启“系统声音采集”，直接响应音乐软件、浏览器等在默认扬声器/耳机
播放的声音；不需要麦克风或立体声混音。直接启动采集模式：
`.\Start-MusicTerrain.cmd -SystemAudio`。系统采集不回放、不保存录音，空格仅暂停画面响应。
拖入文件会返回文件播放模式；切换默认音频设备后请重新开关一次采集。

已移除实验补帧和顶点形变，使用原生方块更新。保留 fast 优化构建、垂直同步与自适应地形更新频率。

左侧导航下方的 `中文 / ENGLISH` 按钮可即时切换语言，无需重新加载音乐或地形。
首次运行在中文字体可用时默认中文；语言选择保存在可执行文件旁的
`musicterrain-language.txt`，下次启动会自动恢复。
中文使用本机系统字体（Windows 优先微软雅黑），不附带或复制系统字体。
其他系统可用 `MUSICTERRAIN_CJK_FONT` 指定支持中文的 TTF/TTC 字体文件。

```powershell
# 静音诊断预览（软件时钟，无声音）
.\Start-MusicTerrain.cmd -Silent
# 指定本次启动语言（不覆盖已保存的选择）
.\Start-MusicTerrain.cmd -Language zh-CN
# 指定音频与群落
.\Start-MusicTerrain.cmd -Audio "D:\Music\song.mp3" -Biome jungle
```

空格播放/暂停，左右键跳转；按住鼠标右键环视，W/A/S/D 移动，Q/E 升降。
画面区域滚轮缩放，右侧滚轮翻阅参数。“设置”页上方提供自动环绕、正交/透视切换，
以及方柱、圆柱、六边形柱、菱形柱选择。自动环绕默认关闭，手动移动会停止环绕。
完整精度默认保持一格体素，128/256 范围不再自动降低精度；可切换性能模式降低负载。

“地下世界”页可调洞穴阈值、洞穴 W 偏移、W 演化速度。洞穴按播放时间推进四维噪波
的 W 切片，默认也受音乐 RMS 的平滑映射影响；暂停会冻结，跳转会重现相应切片。
噪声实验室关闭音乐叠加时可手动观察固定 W 切片。洞穴演化保留底层和洞顶保护。
实时预览/噪声实验室的噪波按钮可循环 8 种类型：Perlin、Simplex、FBM、山脊、
云团、细胞、值噪波、阶梯。

```powershell
.\Start-MusicTerrain.cmd -Shape cylinder -Orbit -Orthographic
.\Start-MusicTerrain.cmd -Shape hexagon -Size 128 -Quality full
```
每次启动使用独立的 `run\session-*` 工作目录；日志在该目录的 `logs` 下。
界面 Save/Load 使用当前会话的预设文件；如需跨会话使用，请自行保留相应 JSON。

## 部署结果

- 程序：`dist\MusicTerrain\musicterrain.exe`，使用 `fast` 优化构建。
- 许可证：`dist\MusicTerrain\LICENSE`；第三方许可另见仓库 `THIRD_PARTY_NOTICES.md`。
- 环境：Windows x64，Rust/Cargo 1.99.1，Visual Studio 2026 C++ 工具链。
- 图形验证：NVIDIA GeForce RTX 3050 Laptop GPU，Vulkan。
- 双击入口会通过独立 PowerShell 进程运行本地启动脚本，无需修改系统执行策略。
- 本机部署不涉及云端地址、端口或服务。

重新构建、测试和部署：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools\deploy-musicterrain-windows.ps1
```

构建使用 `Cargo.lock`、`--profile fast` 和四个并行任务，只构建 `musicterrain` 及其依赖。
首次构建需连接 crates.io；后续可复用 Cargo 缓存。

## 源码结构与数据流

这是 Horizon Cargo 工作区中的独立原生程序，入口位于
`crates/musicterrain/src/main.rs`，提供 `demo`、`preview`、`synthesize`、`analyze` 命令。

| 模块 | 职责 |
| --- | --- |
| `audio.rs` | Symphonia 解码、单声道转换、48 kHz 重采样、内容哈希；限制 20 分钟 PCM |
| `dsp.rs` | RustFFT，默认 4096 点 FFT、512 点步长；提取七频段、RMS、频谱质心、通量及节拍特征 |
| `mapping.rs` / `preset.rs` | 有序映射、响应曲线、attack/release 平滑、参数组合及七个预设 |
| `terrain.rs` | 复用 Horizon 的种子随机数、Perlin/Simplex 噪声构建高度/密度场 |
| `ecology.rs` / `village.rs` / `underground.rs` / `mineshaft.rs` | 群落、植被、村庄、洞穴和矿井几何 |
| `preview.rs` / `visual.rs` | 有界音频导入和网格工作线程、过期任务取消、网格缓存与细节控制 |
| `renderer.rs` / `ui.rs` | 复用 Horizon 的 wgpu 渲染器、相机和 HUD，提供交互编辑器 |
| `transport.rs` | cpal 默认音频设备输出与播放时钟；显式静音模式使用软件时钟 |

实际流程：音频文件 → PCM → 固定步长音频特征 → 映射参数 → 地形场 →
异步体素网格 → GPU 上传与渲染。音频回调不承担解码、FFT 或网格生成。
默认场景内置 65 层原创像素材质，无需准备 Minecraft 资源或安装 Java。

## 本机验收

- `cargo build --locked -p musicterrain`：通过。
- `cargo test --locked -j 4 -p musicterrain`：101 项通过，0 失败。
- 合成音频与分析导出：12 秒，1125 个 FFT 帧，4096 个高度值。
- PGM 文件长度、CSV 行数、元数据和完成标记检查：通过。
- GPU 离屏测试：61 帧，3 个不同网格实际上传，退出码 0。
- 已检查真实渲染截图，地形、材质及界面正常显示。
- 部署程序与构建输出 SHA-256 一致。

证据：`run\validation\tests.log`、`run\validation\results.json`；
截图及导出位于 `run\smoke-456d58ac91e14b059d0c8facddf63a2f`。
截图验证采用静音软件时钟；未进行实际扬声器试听或长时间性能测试。
自动运行遵守根目录 `AGENTS.md` 的数据隔离和禁用声音要求。

## 当前限制

已支持 Windows 默认输出设备的系统声音回环采集；麦克风采集、Minecraft 世界存档导出、碰撞与流体模拟尚未实现。
`analyze` 输出 PGM/CSV/JSON，仅包含核心高度场，不包含装饰后的完整场景。
128/256 完整精度需要更多计算，地形更新可能慢于画面帧率；建议先保持默认 64×64。
当前交付为本机开发构建；正式分发可另用优化构建并进行音频、长时间运行及目标机器验收。
更完整限制见 `KNOWN_ISSUES.md`。

## Language verification

Chinese/English UI: 92 tests passed, including all eight pages and real Chinese glyph rasterization.
Headless analysis: 51 tests passed. Format and Clippy checks passed.
Both languages rendered successfully on the deployed Vulkan executable.
Evidence: `run/validation/language-results.json`.

## Performance build

Deployment now uses the optimized `fast` profile (release optimizations without
LTO), including the shared renderer and meshing dependencies. Normal preview
requests actual voxel mesh updates up to 60 Hz, with an adaptive interval based
on measured generation cost; it keeps the bounded worker and stale-result checks.
The ordinary window uses VSync/FIFO rather than uncapped mailbox rendering.
Offscreen verification remains uncapped to measure throughput separately.

Diagnostics includes actual terrain update frequency, distinct from display FPS.
One-second performance logs include mean, p95, p99 and maximum frame intervals,
mesh completion frequency, generation and upload timing. Detailed per-mesh logs
are at debug level so ordinary playback does not synchronously log every mesh.
Experimental transition blending and vertex deformation have been removed entirely.

Measured comparison on this RTX 3050 machine, using the same synthesized clip,
default seed/preset and 64×64 extent (11-second timed offscreen runs; medians of
one-second records at audio times 1..9):

| Metric | Previous deployment | Optimized deployment |
| --- | ---: | ---: |
| Actual mesh completions per second | 14.125 | 34.908 |
| Mesh generation, ms | 35.100 | 22.462 |
| Mean frame interval, ms | 7.176 | 3.841 |

A separate normal-window smoke completed 301 frames successfully with VSync:
about 143 display FPS and 45 terrain updates/second in two short steady records.
These short measurements do not guarantee sustained performance for other audio,
presets, scene extents or temperatures. Voxel occupancy still changes discretely.
Evidence: `run/validation/performance-comparison.json`; normal-window screenshot
and logs: `run/optimized-window-27ff48fa72894eff9c507bf1ed937c0b`.

## 本次洞穴、外形与相机更新验收

101 项 MusicTerrain 测试及共享渲染器的投影中心回归测试通过；Clippy（警告视为错误）、
优化构建和差异空白检查通过。四组离屏 GPU 测试覆盖透视、正交、圆柱、六边形、
128/256 完整精度与新增噪波，均成功保存截图并以 0 退出。
另有 901 帧真实窗口测试：自动环绕、正交、圆柱剖切场景共完成 300 次网格更新。
这些验证使用独立会话目录和静音诊断模式。

证据：`run/validation/cave-camera-tests.log`、`cave-camera-clippy.log`、
`projection-renderer-tests.log`、`cave-camera-gpu.json`、`cave-camera-window.json`。

2026-10-31 系统声音采集更新：103 项测试、Clippy 和优化构建通过。
真实 WASAPI 回环测试使用 48 kHz 双声道默认输出设备，1201 帧 GPU 渲染后正常退出，
截图中系统音频 RMS 为 0.299，频谱和地形随输入更新。
证据：`run/validation/capture-tests.log`、`capture-clippy.log`、`system-capture-gpu.json`。

2026-10-31 完整精度性能更新：体素逐列连续存储、最多四线程填充/网格生成、
未变化 GPU 区段复用。105 项测试及串行/并行几何一致性检查通过。
本机短测中，128 地形更新约 13.45 → 20.93 次/秒，256 约 3.00 → 4.97 次/秒；
画面帧率均约 144 FPS。256 完整精度仍有明显计算成本。
条件和限制见 `PERFORMANCE.md`，数据见 `run/validation/full-detail-comparison.json`。
