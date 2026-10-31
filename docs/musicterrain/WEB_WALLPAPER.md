# MusicTerrain 网页壁纸

这是 Wallpaper Engine 的 **Web 类型**项目：Rust/WASM 生成体素和网格，
网页使用 WebGL 2 绘制，音频通过 Wallpaper Engine 的频谱回调接入。
运行壁纸时不需要 EXE、本地服务器、麦克风、Node 或 Rust 工具链，也不请求联网资源。

![网页壁纸实际预览](../../web/musicterrain/preview.jpg)

## 使用

本机项目已构建时，双击根目录 `Start-MusicTerrain-Web.cmd` 可在 Wallpaper Engine 中打开。
加 `-Preview` 可打开独立预览窗口。安装目录不在默认 Steam 库时，可使用
`-WallpaperEngine "E:\SteamLibrary\steamapps\common\wallpaper_engine"`。

分享导入包：`dist/MusicTerrain-Web.zip`。解压为独立文件夹，整体复制到 Wallpaper Engine
安装目录的 `projects/myprojects/`，保留包内的 `project.json` 和自定义属性，
再在“已安装”列表选择本项目。也可在编辑器“创建壁纸”入口导入 `index.html`。
不要导入整个 Rust 仓库。
所有运行文件都位于 `dist/MusicTerrain-Web`，可以离线复制使用。
原始程序设置面板默认隐藏，通过 Wallpaper Engine 的属性栏调整参数。

支持：

- 32 / 64 / 96 / 128 / 256 范围，完整方块精度或自适应精度，默认 64。
- 方柱、圆柱、六边形柱、菱形柱；八种原有噪波。
- 系统音乐响应、W 维洞穴演化、洞顶和底层保护。
- 自动环绕、正交/透视、视角缩放、天空颜色。
- 缩放支持 0.10～4.00（步进 0.01），可直接输入 0.1、0.2、0.3、0.4。
- X/Y/Z 数值调整视角位置，中键或 Shift + 右键拖动平移，一键重置视角。
- 与原生版共用的 11 种群落、完整树木花草、65 种原创材质和半透明水体。
- 村庄、房屋、农田、水井、道路，地下矿井、铁轨、支架和地下群落。
- 完整 / 剖开一角 / 仅地下视图；鼠标右键调整视角，滚轮缩放。
- 全部地形参数、7 个原生预设、Noise Lab 三种切片和完整音频映射编辑。
- 原生预设 JSON 导入 / 导出、本机保存、撤销 / 重做和本地纹理文件夹。
- 本地音乐文件播放器，使用与原生版相同的 Rust FFT 分析器。
- Wallpaper Engine 全局 FPS、暂停/恢复和音频开关。

普通浏览器只提供预览，不会获取系统声音。“模拟音乐”是明确标注的无声测试输入；
在 Wallpaper Engine 中使用真实系统音频，并隐藏该测试按钮。
系统音频模式不保存或播放音频。选择本地文件后，点击播放器才会播放声音，
可暂停、拖动进度或点击“切回系统音乐”。浏览器对具体编码的支持决定文件是否可播放。
在壁纸属性中打开“显示画面设置按钮”，点击画面右下角的齿轮进入完整编辑面板。
桌面若拦截鼠标操作，可使用 `Start-MusicTerrain-Web.cmd -Preview` 的独立窗口编辑。
普通浏览器开发预览需通过本地 HTTP 服务打开，以符合 Worker 的来源限制；
Wallpaper Engine 导入后直接加载本地项目，不需要服务器。

## 构建

需要现有 Rust 工具链及 WASM 目标，开发验证另需 Node；使用者不需要这些工具。

```powershell
rustup target add wasm32-unknown-unknown
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools\build-musicterrain-web.ps1
# 同时安装为本机新项目；已有同名目录时创建新目录，不覆盖用户项目。
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools\build-musicterrain-web.ps1 -Install
```

打包脚本只复制 `web/musicterrain` 中的独立运行文件、许可证和生成的 WASM。
WASM 嵌入本地 `kernel-base64.js`，避免 `file://` 下额外的 WASM fetch/MIME 限制。
包中不包含 Minecraft 资源，也没有 CDN 或原生程序桥接。

## 实现

`crates/musicterrain-web` 使用 `musicterrain` 的 `scene` 构建，直接调用原生版的
`visual::mesh_snapshot_with_options`。地形、群落、植被、村庄、矿井、地下装饰、
材质、噪波、4D 洞穴、参数和映射共用同一套 Rust 代码。
独立的 CPU 网格容器避免把 wgpu / winit 引入 WASM；原生版仍使用 Horizon 的生产网格格式。

WASM 在一个 Web Worker 中执行。只允许一个网格任务执行，属性版本变化后丢弃旧结果。
传输网格使用可转移缓冲区，暂停时停止调度；渲染与地形更新分开运行。
更新间隔同时受用户上限和最近生成耗时约束，避免大范围占满后台 CPU。
网格保留原生体素轮廓、固定点 UV 和面颜色；没有顶点形变或实验补帧。
浏览器直接上传每顶点 24 字节的压缩四边形，使用共享索引绘制，
取代每面 6 个、每个 36 字节的浮点顶点；每次顶点传输量减少约 56%。
GPU 顶点缓冲交替复用，仅容量不足时扩容。相机调整不会使待完成的地形失效或强制重建。
面板分别显示画面 FPS、宿主帧率上限、地形更新频率、生成和上传耗时。
网页遵守 Wallpaper Engine 全局 FPS 设置；若上限为 15，需在“设置 → 性能”调高，
代码优化无法越过这个上限。提高全局上限也会影响其他壁纸。

Wallpaper Engine 提供的是双声道各 64 个频谱幅值，并非 PCM。
网页保留 64 点信息，聚合为 7 个近似频段、整体强度、频谱重心、通量和起音估计，
在连续稳定起音后给出节拍相位。这些不能等同于原生 PCM FFT 的精确 RMS/频段。
本地文件模式则通过 Web Audio 取 4096 点 PCM 窗口，交给共享 Rust `Analyzer` 分析；
它是流式分析，跳转时重置历史，不是原生整首离线预分析的确定性时间线。
音频停止或回调中断后强度归零；音乐参数仍使用共享的 attack/release 映射。
默认启用自动音频归一化，使用快速跟随、缓慢释放的公共增益，保留节拍起伏，
避免 QQ 音乐等较弱或稀疏的频谱经过映射后几乎不可见。
整体强度也驱动地形振幅，因此低频较少的音乐仍可响应。
可调整“音频灵敏度”或关闭自动归一化；设置面板会显示原始输入与实际响应值。

## 平台边界

本版已移植原生场景和主要编辑功能，运行宿主仍为网页：系统音频由 Wallpaper Engine
提供，文件解码、播放器、文件选择和下载由浏览器提供。没有原生设备选择器、原生窗口、
wgpu GPU 计时、整首离线分析导出或自由飞行键盘控制；界面为中文。
Minecraft 世界存档导出在原生版也尚未实现。
纹理文件不会写入项目或打包；重启后需重新选择。自定义预设可手动保存到本机存储，
跨项目迁移建议导出 JSON。下载受宿主限制时可直接复制面板中的 JSON。
水体为静态填充，没有流体模拟；透明水面没有逐三角形距离排序。
128 / 256 为高负载选项，浮岛、完整精度和大量装饰还会增加耗时。
WASM 场景在一个 Worker 内运行，未使用原生版的多线程加速；不能把相机 FPS 当作地形更新频率。
自适应精度沿用原生规则，步长增大时会省略部分结构。
无效参数或映射会保留上一帧场景并显示错误，修正或撤销后继续生成。

## 验证

```powershell
cargo test --locked -j4 -p musicterrain -p musicterrain-web
cargo clippy --locked -j4 -p musicterrain -p musicterrain-web --all-targets -- -D warnings
node --test tools/test-musicterrain-web.cjs
# 需要本机 Edge；Playwright 仅作为开发依赖安装到忽略的 run 目录。
npm.cmd install --prefix run/validation/web-tools --no-save playwright
node tools/verify-musicterrain-web.cjs
```

浏览器测试验证实际 WASM 加载和 WebGL 绘制、音频回调、属性切换、四种外形、
暂停冻结、静音归零、10 FPS 限制和超宽屏/128 范围，还检查村庄 / 矿井、7 个预设、
噪波切片、无效配置恢复，以及合成 500 Hz 文件播放经过真实 Web Audio → Rust FFT 的路径。
测试浏览器使用静音启动参数，不播放测试声音。系统音频接口使用测试频谱注入，
不代表已经测量真实系统音频端到端延迟。
证据位于 `run/validation/web-browser`，截图为实际浏览器画面。

2026-10-31 验证结果：105 项原生/共享内核测试、5 项网页 Rust 内核测试、
5 项 JavaScript 桥接/相机测试及 Edge 集成检查通过，Clippy 警告视为错误检查通过。
弱音回归使用仅两个频点、幅值 0.002 的测试频谱，验证它能驱动真实 WASM 地形变化，
并验证静音归零、较弱音量可检测及释放阶段保留强弱起伏。
生成的 WASM 没有宿主导入。已通过命令行安装、打开 Wallpaper Engine 项目及预览窗口，
配置中确认选中了本项目，验证期间没有新增引擎加载日志错误。
Windows 界面自动化连接不可用，因此画面检查使用真实浏览器截图，
没有把模拟频谱测试声称为 Wallpaper Engine 实机音频延迟验收。

官方接口参考：
[音频频谱](https://docs.wallpaperengine.io/en/web/audio/visualizer.html)、
[属性与暂停](https://docs.wallpaperengine.io/en/web/api/propertylistener.html)、
[FPS 限制](https://docs.wallpaperengine.io/en/web/performance/fps.html)、
[导入网页壁纸](https://docs.wallpaperengine.io/en/web/first/gettingstarted.html)。
