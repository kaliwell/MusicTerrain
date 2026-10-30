//! UI-only translations; preset identifiers, exported data and filenames remain stable.
use std::{borrow::Cow, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    English,
    #[default]
    Chinese,
}

impl Language {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "zh" | "zh-CN" => Some(Self::Chinese),
            "en" | "en-US" => Some(Self::English),
            _ => None,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::Chinese => "zh-CN",
            Self::English => "en",
        }
    }
    fn path() -> std::io::Result<PathBuf> {
        Ok(std::env::current_exe()?.with_file_name("musicterrain-language.txt"))
    }
    pub fn load() -> Self {
        let saved = Self::path()
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| Self::parse(&s));
        saved.unwrap_or_else(|| {
            if crate::unicode_font::available() {
                Self::Chinese
            } else {
                Self::English
            }
        })
    }
    pub fn save(self) -> std::io::Result<()> {
        std::fs::write(Self::path()?, self.code())
    }
    pub fn toggle(self) -> Self {
        match self {
            Self::English => Self::Chinese,
            Self::Chinese => Self::English,
        }
    }
}

pub fn translate(language: Language, text: &str) -> Cow<'_, str> {
    if language == Language::English {
        return Cow::Borrowed(text);
    }
    if [".wav", ".mp3", ".flac"]
        .iter()
        .any(|ext| text.to_ascii_lowercase().ends_with(ext))
    {
        return Cow::Borrowed(text);
    }
    if let Some((_, translated)) = LABELS
        .iter()
        .find(|(key, _)| text.eq_ignore_ascii_case(key))
    {
        return Cow::Borrowed(translated);
    }
    // Formatted labels retain their values; match whole tokens to protect user filenames.
    let mut result = String::new();
    let mut offset = 0;
    while offset < text.len() {
        let rest = &text[offset..];
        let matched = LABELS.iter().find(|(key, _)| {
            rest.get(..key.len())
                .is_some_and(|s| s.eq_ignore_ascii_case(key))
                && (offset == 0
                    || !text[..offset]
                        .chars()
                        .next_back()
                        .unwrap()
                        .is_alphanumeric())
                && rest[key.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !c.is_alphanumeric())
        });
        if let Some((key, value)) = matched {
            result.push_str(value);
            offset += key.len();
        } else {
            let c = rest.chars().next().unwrap();
            result.push(c);
            offset += c.len_utf8();
        }
    }
    Cow::Owned(result)
}

// Longer phrases precede individual words used in dynamic diagnostic labels.
const LABELS: &[(&str, &str)] = &[
    ("4D W / MUSIC REACTIVE CAVES", "4D W 维 / 音乐响应洞穴"),
    ("MINES REQUIRE FULL HEIGHT FIELD", "矿井需完整精度高度场"),
    ("SHAPE", "地形外形"),
    ("SQUARE", "方柱"),
    ("CYLINDER", "圆柱"),
    ("HEXAGON", "六边形柱"),
    ("DIAMOND", "菱形柱"),
    ("DETAIL FULL: 1 BLOCK", "完整精度：1 格"),
    ("DETAIL ADAPTIVE", "性能模式：降低精度"),
    ("VILLAGES: ENABLE FULL DETAIL", "村庄需完整精度或 64 范围"),
    ("TERRAIN UPDATES", "地形更新"),
    ("ANALYZING AUDIO...", "正在分析音频..."),
    ("LOADING...", "正在加载..."),
    ("TEXTURES INITIALIZING", "正在初始化材质"),
    (
        "AUDIO PLAYING - DROP ANOTHER FILE TO IMPORT",
        "正在播放 · 可拖入其他音乐",
    ),
    (
        "AUDIO DEVICE FAILED - PLAYBACK STOPPED",
        "音频设备故障 · 播放已停止",
    ),
    (
        "AUDIO IMPORT QUEUE FULL - WAIT FOR CURRENT IMPORT",
        "导入队列已满 · 请稍候",
    ),
    (
        "NO AUDIO OUTPUT - USE --SILENT-PREVIEW FOR A SOFTWARE CLOCK",
        "无音频输出 · 可使用静音预览",
    ),
    ("AUDIO DEVICE ERROR", "音频设备错误"),
    ("IMPORT ERROR", "导入错误"),
    (
        "DROP MP3 / WAV / FLAC TO IMPORT",
        "拖入 MP3 / WAV / FLAC 导入音乐",
    ),
    ("SYNTHESIZED 12 SECOND DEMO", "内置合成音乐 · 12 秒"),
    (
        "SILENT SOFTWARE CLOCK - NO SOUND",
        "静音模式 · 软件时钟 · 无声音",
    ),
    (
        "SILENT PREVIEW - SOFTWARE CLOCK (NO AUDIO OUTPUT)",
        "静音预览 · 软件时钟（无音频输出）",
    ),
    ("AUDIO OUTPUT CLOCK", "音频输出时钟"),
    ("01 LIVE", "01 实时预览"),
    ("02 MUSIC WORLD", "02 音乐世界"),
    ("03 NOISE LAB", "03 噪声实验室"),
    ("04 MAPPINGS", "04 参数映射"),
    ("05 PRESETS", "05 预设"),
    ("06 SETTINGS", "06 设置"),
    ("07 DIAGNOSTICS", "07 性能诊断"),
    ("08 UNDERGROUND", "08 地下世界"),
    ("TEXTURE SOURCE", "材质来源"),
    ("ORIGINAL MATERIALS", "原创材质"),
    ("LOCAL PACK", "本地资源包"),
    ("FALLBACK", "内置补全"),
    ("RMB + MOUSE LOOK", "右键 + 鼠标：环视"),
    ("WASD / Q E FLY", "WASD 移动 / Q E 升降"),
    ("SPACE PLAY / PAUSE", "空格：播放 / 暂停"),
    ("LEFT RIGHT SEEK", "左右键：跳转进度"),
    ("CTRL Z / Y HISTORY", "Ctrl Z / Y：撤销 / 重做"),
    ("SCROLL FOR MORE PARAMETERS", "滚轮查看更多参数"),
    (
        "SCROLL RULES - ORDER IS STABLE",
        "滚轮查看规则 · 按顺序执行",
    ),
    ("VISUAL TARGET NOT IMPLEMENTED", "此可视化目标尚未实现"),
    ("CHOOSE ACTIVE TARGET", "选择可用目标"),
    ("DELETE RULE", "删除规则"),
    ("INPUT MIN", "输入下限"),
    ("INPUT MAX", "输入上限"),
    ("OUTPUT MIN", "输出下限"),
    ("OUTPUT MAX", "输出上限"),
    ("ATTACK S", "起音秒数"),
    ("RELEASE S", "释音秒数"),
    ("CLAMP ON", "钳制：开"),
    ("CLAMP OFF", "钳制：关"),
    ("MUSIC OVERLAY ON", "音乐叠加：开"),
    ("MUSIC OVERLAY OFF", "音乐叠加：关"),
    ("SPECTRUM ON", "频谱显示：开"),
    ("SPECTRUM OFF", "频谱显示：关"),
    ("AV OFFSET SECONDS", "音画偏移（秒）"),
    ("AUDIO OUTPUT: DEFAULT DEVICE", "音频输出：默认设备"),
    ("CAPTURE: NOT IMPLEMENTED", "系统声音采集：尚未实现"),
    ("TREES / PLANTS ON", "树木 / 植物：开"),
    ("TREES / PLANTS OFF", "树木 / 植物：关"),
    ("WATER ON", "水体：开"),
    ("WATER OFF", "水体：关"),
    ("CAVES ON", "洞穴：开"),
    ("CAVES OFF", "洞穴：关"),
    ("VILLAGES: HEIGHT FIELD ONLY", "村庄仅支持高度场预览"),
    ("VILLAGES: SELECT 64 X 64", "村庄需选择 64 X 64 范围"),
    ("ORIGINAL MODERN-STYLE CAVES", "原创现代风格洞穴"),
    ("CHAMBERS + WINDING TUNNELS", "大型洞厅 + 曲折洞道"),
    ("LUSH / DRIPSTONE / DEEP", "繁茂 / 溶洞 / 深层群落"),
    ("LOW AQUIFERS + ORE VEINS", "地下含水层 + 矿脉"),
    ("RAIL SEGMENTS", "铁轨段数"),
    ("MINES REQUIRE 64 HEIGHT FIELD", "矿井需 64 范围高度场"),
    ("CAVES OFF ALSO DISABLES MINES", "关闭洞穴也会禁用矿井"),
    ("PREVIEW Y 0..128, NO SAVE", "预览高度 0..128，不生成存档"),
    ("MUSIC WORLD: UNAVAILABLE", "音乐世界：暂不可用"),
    ("SAVE ADAPTER NOT IMPLEMENTED", "世界存档导出尚未实现"),
    ("NO WORLD FILES ARE WRITTEN", "不会写入世界存档文件"),
    (
        "PRESET SAVED (EXISTING FILES ARE PRESERVED)",
        "预设已保存（保留已有文件）",
    ),
    ("PRESET LOADED", "预设已加载"),
    ("HEIGHT XZ", "高度图 XZ"),
    ("DENSITY Y=20", "密度 Y=20"),
    ("SECTION Z=0", "剖面 Z=0"),
    ("NOISE LAB", "噪声实验室"),
    ("Bass Mountains", "低音山脉"),
    ("Wave Terrain", "波浪地形"),
    ("Noise Storm", "噪声风暴"),
    ("Floating Islands", "浮空群岛"),
    ("Crystal Pulse", "水晶脉冲"),
    ("Ambient Landscape", "氛围景观"),
    ("Extreme Terrain", "极限地形"),
    ("UPLOAD ENQUEUE", "上传入队"),
    ("RENDER ENCODE", "渲染编码"),
    ("RENDER WAIT", "渲染等待"),
    ("UPLOAD BYTES", "上传字节"),
    ("RESIDENT SECTIONS", "驻留区段"),
    ("MESH AGE", "网格延迟"),
    ("QUEUE CAPACITY 1 - BUSY", "队列容量 1 · 忙碌"),
    ("MATERIAL KINDS", "材质种类"),
    ("SOLID VOXELS", "实体体素"),
    ("CARVED VOXELS", "挖空体素"),
    ("CAVE FACES", "洞穴面数"),
    ("WATER QUADS", "水体面数"),
    ("BaseHeight", "基础高度"),
    ("DomainWarp", "域扭曲"),
    ("DensityThreshold", "密度阈值"),
    ("CaveThreshold", "洞穴阈值"),
    ("CaveW", "洞穴 W 偏移"),
    ("CaveSpeed", "W 演化速度"),
    ("AUTO ORBIT ON", "自动环绕：开"),
    ("AUTO ORBIT OFF", "自动环绕：关"),
    ("ORTHOGRAPHIC VIEW", "正交视角"),
    ("PERSPECTIVE VIEW", "透视视角"),
    ("WHEEL ZOOM / RMB LOOK", "滚轮缩放 / 右键环视"),
    ("Billow", "云团"),
    ("Cellular", "细胞"),
    ("Value", "值噪波"),
    ("Terraced", "阶梯"),
    ("RidgeStrength", "山脊强度"),
    ("ErosionInfluence", "侵蚀影响"),
    ("StructureDensity", "结构密度"),
    ("IslandStrength", "浮岛强度"),
    ("ColorShift", "颜色偏移"),
    ("ParticleIntensity", "粒子强度"),
    ("BeatStrength", "节拍强度"),
    ("BeatPhase", "节拍相位"),
    ("EffectIntensity", "效果强度"),
    ("Shockwave", "冲击波"),
    ("Frequency", "频率"),
    ("Amplitude", "振幅"),
    ("Octaves", "倍频层数"),
    ("Persistence", "持续度"),
    ("Lacunarity", "频率倍增"),
    ("Detail", "细节"),
    ("Scale", "缩放"),
    ("Gain", "增益"),
    ("Bias", "偏置"),
    ("Smoothstep", "平滑曲线"),
    ("Logarithmic", "对数曲线"),
    ("Linear", "线性"),
    ("Power", "幂曲线"),
    ("Replace", "替换"),
    ("Multiply", "相乘"),
    ("Add", "相加"),
    ("Centroid", "频谱质心"),
    ("Onset", "瞬态"),
    ("Flux", "频谱通量"),
    ("Band", "频段"),
    ("Birch-Forest", "桦木林"),
    ("Birch Forest", "桦木林"),
    ("Tropical Jungle", "热带丛林"),
    ("Meadow", "草甸"),
    ("Forest", "森林"),
    ("Alpine", "高山"),
    ("Shore", "海岸"),
    ("Taiga", "针叶林"),
    ("Desert", "沙漠"),
    ("Badlands", "恶地"),
    ("Savanna", "热带草原"),
    ("Swamp", "沼泽"),
    ("Jungle", "丛林"),
    ("Showcase", "展示"),
    ("Auto", "自动"),
    ("Surface", "地表"),
    ("Cutaway", "剖切"),
    ("Underground", "地下切片"),
    ("Ridged", "棱脊"),
    ("PAUSE", "暂停"),
    ("PLAY", "播放"),
    ("SAVE", "保存"),
    ("LOAD", "加载"),
    ("RESET", "重置"),
    ("UNDO", "撤销"),
    ("REDO", "重做"),
    ("COPY", "复制"),
    ("DEL", "删除"),
    ("NOISE", "噪声"),
    ("SEED", "种子"),
    ("PREVIEW", "预览范围"),
    ("BIOME", "群落"),
    ("VILLAGES", "村庄"),
    ("HOUSES", "房屋"),
    ("FARMS", "农田"),
    ("MINES", "矿井"),
    ("SUPPORTS", "支架"),
    ("RAILS", "铁轨"),
    ("VIEW", "视图"),
    ("TREES", "树木"),
    ("PLANTS", "植物"),
    ("GENERATION", "生成耗时"),
    ("REVISION", "修订"),
    ("SUBMITTED", "已提交"),
    ("COMPLETED", "已完成"),
    ("DROPPED", "已丢弃"),
    ("FRAME", "帧耗时"),
    ("SILENCE", "静音"),
    ("TRUE", "是"),
    ("FALSE", "否"),
    ("N/A", "不可用"),
    ("ON", "开"),
    ("OFF", "关"),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels_and_dynamic_values_switch_without_mutating_identifiers() {
        assert_eq!(translate(Language::Chinese, "06 SETTINGS"), "06 设置");
        assert_eq!(translate(Language::Chinese, "BIOME FOREST"), "群落 森林");
        assert_eq!(translate(Language::Chinese, "NOISE Ridged"), "噪声 棱脊");
        assert_eq!(
            translate(Language::Chinese, "GENERATION 28.42 MS"),
            "生成耗时 28.42 MS"
        );
        assert_eq!(translate(Language::English, "06 SETTINGS"), "06 SETTINGS");
        assert_eq!(
            translate(Language::Chinese, "myForestSong.wav"),
            "myForestSong.wav"
        );
        assert_eq!(
            translate(Language::Chinese, "中文文件名.mp3"),
            "中文文件名.mp3"
        );
        assert_eq!(
            Language::parse("zh-CN").unwrap().toggle(),
            Language::English
        );
    }
}
