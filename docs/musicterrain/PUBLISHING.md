# 发布 MusicTerrain 网页壁纸

使用已有的独立网页项目发布，可以保留全部音频和设置属性。
本机项目位于 Wallpaper Engine 安装目录的 `projects/myprojects/musicterrain-web`。
可分发的项目副本位于 `dist/MusicTerrain-Web`，备份包为 `dist/MusicTerrain-Web.zip`。

## 创意工坊发布

1. 打开 Wallpaper Engine 的壁纸编辑器，打开现有的 MusicTerrain 网页项目。
2. 在顶部“创意工坊”菜单选择“准备发布”或“在创意工坊分享壁纸”。
3. 填写标题、描述，年龄评级选择适合所有人，类型保持 Web。
   可选标签：音频响应、3D、自然、放松。
4. 预览图选择 `dist/MusicTerrain-Publish/preview.gif`。
5. 选择公开、好友或私有可见性，检查详情后点击“发布”。

发布由你在 Steam 界面完成，本次工具只生成项目文件和展示素材。
后续在同一个项目中再次发布即可更新原有作品。
请保留发布后的项目目录和其中的 `project.json`，它包含作品的关联信息。
创意工坊发布不代替项目源文件备份。

## 展示素材

- `preview.gif`：128 × 128，12 秒循环动态封面，小于 500 KB。
- `demo.gif`：640 × 426，12 秒循环展示图。
- `demo.mp4`：960 × 640，12 秒 H.264 展示视频，无音轨。
- `poster.png`：960 × 640 静态截图。

素材在 `dist/MusicTerrain-Publish`，与实际壁纸运行文件分开放置。
这些画面由同一套 Rust/WASM 场景和 WebGL 渲染器生成，没有使用 AI 图片。
音乐响应使用合成测试节奏，不包含系统音频采集、个人音乐或 QQ 音乐录音。
GIF 的帧率为压缩展示素材而降低，不代表壁纸实时运行帧率。

## 可复制的发布文案

标题：MusicTerrain · 音乐方块地形 / Audio Reactive Voxel Terrain

描述：

让音乐塑造一座方块世界。通过 Wallpaper Engine 的系统音频分析，
地形、颜色和四维洞穴随音乐变化。

包含 11 种群落、65 种原创像素材质、村庄农田和地下矿井，
支持圆柱、六边形等外形、自动环绕、正交视角和自由调整取景位置。
可编辑地形参数、音频映射和预设，也可导入本地音乐进行分析。

建议从 64 范围开始；128 / 256、浮岛和完整精度会增加负载。
在 Wallpaper Engine 的“设置 → 性能”中设置合适的 FPS 上限。
打开“显示画面设置按钮”，点击右下角齿轮进入完整编辑面板。
播放系统音乐时请启用壁纸的“音频处理”。

基于 Horizon，使用 Rust/WASM 和 WebGL 2；原创素材随项目离线提供。
项目源码与 MIT 许可证：https://github.com/kaliwell/MusicTerrain

## 官方参考

- [网页项目导入与发布](https://docs.wallpaperengine.io/en/web/first/gettingstarted.html)
- [发布与动态预览图规格](https://docs.wallpaperengine.io/en/scene/first/publishing.html)

官方允许 GIF 动态封面，最大 256 × 256、1 MB；推荐 128 × 128、小于 500 KB。

## 重新生成素材

先构建网页包，再安装开发用 Playwright（见网页壁纸文档）。
本机需要 Edge、Node 和 FFmpeg，壁纸使用者不需要这些开发工具。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/build-musicterrain-promo.ps1
```
