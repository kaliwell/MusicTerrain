MusicTerrain · 音乐方块地形（共享原生 Rust 场景）

使用：将解压后的整个文件夹放入 Wallpaper Engine 安装目录的 projects/myprojects/ 下，
然后在 Wallpaper Engine 的“已安装”列表中选择本项目。
本包已包含 project.json 及全部自定义属性，请保留这些文件。
也可通过 Wallpaper Engine 编辑器导入 index.html。

在 Wallpaper Engine 中播放系统音乐即可响应；普通浏览器只提供预览。
默认 64 范围，每格一个方块。128 / 256 为高负载选项，可选择自适应精度。
可在壁纸属性中设置外形、噪波、洞穴、环绕、正交视角、缩放和更新频率。
完整场景包含 11 种群落、65 种原创材质、植被、村庄农田、矿井和地下群落。
开启“显示画面设置按钮”后点击齿轮，进入参数、噪波实验室、映射和预设编辑。
支持原生预设 JSON、本机预设保存、本地纹理和本地音乐播放器。
本地音乐点击播放才会发声；系统音频模式只分析声音，不重复播放。
网页未包含原生设备选择、整首离线分析导出或 Minecraft 世界存档导出。

无需额外 EXE、Node、Rust、麦克风或常驻本地服务器；运行资源全部离线。
源码：https://github.com/kaliwell/MusicTerrain
许可证见 LICENSE。
