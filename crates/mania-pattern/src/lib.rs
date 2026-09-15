//! 基于 osumania_map_analyser 规则的 mania 键型分析（Rust 移植）。
//!
//! 上游是 JavaScript 实现，这里是按固定版本逐条对照移植的 Rust 版本，模块划分与上游
//! 文件一一对应：
//!
//! | 上游 | 这里 |
//! | --- | --- |
//! | `js/parser/patternOsuParser.js` | [`parser`] |
//! | `js/patterns/primitives.js` | [`primitives`] |
//! | `js/patterns/patternsDef.js` | [`patterns`] |
//! | `js/patterns/clustering.js` | [`clustering`] |
//! | `js/patterns/categorise.js` | [`categorise`] |
//! | `js/patterns/summary.js` | [`summary`] |
//! | `js/patterns/config.js` | [`config`] |
//!
//! 移植目标是与上游**逐字段一致**：解析顺序、时间取整、聚类顺序、稳定性排序都保持一致，
//! 因此不能用「结果看起来差不多」来判断改动是否正确。

pub mod categorise;
pub mod chart;
pub mod clustering;
pub mod config;
pub mod features;
pub mod parser;
pub mod patterns;
pub mod primitives;
pub mod rate;
pub mod summary;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

pub use clustering::{PatternCluster, specific_clusters};
pub use features::MmaFeatures;
pub use rate::ManiaGameMod;
pub use summary::MmaReport;

/// 上游固定提交，供数据集记录来源。
pub const MANIA_MMA_SNAPSHOT: &str =
    "osumania_map_analyser@70e2bd92524e093ee94ca9cb6cc159ec223faa04";

/// 本移植的算法标识；键型规则、参数或派生特征口径变化时必须提升版本。
pub const MANIA_MMA_ALGORITHM_ID: &str = "mania-mma-pattern-v1";

/// 分析产物，尚未绑定谱面身份。
#[derive(Debug, Clone)]
pub struct ManiaMmaAnalysis {
    pub key_count: u8,
    pub report: MmaReport,
    pub features: MmaFeatures,
}

/// 分析一段 mania `.osu` 文本。
///
/// `game_mod` 决定时钟倍率：DT/HT 会先按倍率缩放谱面再分析，不使用 NoMod 特征估算。
pub fn analyze(text: &str, game_mod: ManiaGameMod) -> Result<ManiaMmaAnalysis> {
    let meta = lightweight_metadata(text);
    if meta.mode != Some(3) {
        bail!("only osu!mania mode is supported");
    }
    let Some(key_count) = meta.key_count else {
        bail!("missing CircleSize");
    };
    if !matches!(key_count, 4 | 6 | 7) {
        bail!("only 4K, 6K, and 7K are supported (found {key_count}K)");
    }

    let transformed = rate::transform_rate(text, game_mod.rate())?;
    let chart = parser::parse_osu_mania(&transformed)?;
    if chart.keys != key_count as usize {
        bail!(
            "key count changed while transforming: {} -> {}",
            key_count,
            chart.keys
        );
    }

    let report = summary::from_chart(&chart);
    let features = features::derive(&chart, &report);

    Ok(ManiaMmaAnalysis {
        key_count,
        report,
        features,
    })
}

/// 只读取模式与键数的最小元数据，避免为了校验再跑一遍完整分析器。
#[derive(Debug, Default)]
struct LightMetadata {
    mode: Option<u8>,
    key_count: Option<u8>,
}

fn lightweight_metadata(text: &str) -> LightMetadata {
    let mut out = LightMetadata::default();
    let mut section = String::new();
    for line in text.split('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed.to_owned();
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match (section.as_str(), key) {
            ("[General]", "Mode") => out.mode = value.parse::<u8>().ok(),
            ("[Difficulty]", "CircleSize") => {
                out.key_count = value
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .map(|value| value.round() as u8);
            }
            _ => {}
        }
    }
    out
}

/// 可持久化的键型记录，供数据集文件与消费方使用。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManiaMmaRecord {
    pub beatmap_id: u64,
    pub beatmapset_id: u64,
    pub game_mod: ManiaGameMod,
    pub key_count: u8,
    /// `RC` / `HB` / `Mix` / `LN`。
    pub mode_tag: String,
    /// 上游 `Category`，例如 `Shield`、`Jumpstream/Handstream Tech`。
    pub category: String,
    /// 重要度最高的簇所属的六大类。
    pub dominant_pattern: String,
    pub ln_note_ratio: f32,
    pub hb_row_ratio: f32,
    pub sv_amount: f32,
    pub duration_seconds: f32,
    /// 六类覆盖率，顺序为 Stream、Chordstream、Jacks、Coordination、Density、Wildcard。
    pub coverage: [f32; 6],
    pub clusters: Vec<ManiaMmaCluster>,
    pub bars: Vec<ManiaMmaBar>,
    pub subtypes: Vec<(String, f32)>,
    /// 平均 NPS、峰值 NPS、持续 NPS、最长持续段秒数。
    pub intensity: [f32; 4],
    /// 持续段占比、最长持续段占全谱比例、空窗比例。
    pub temporal: [f32; 3],
    /// 每个键型在相邻时间窗之间的平均变化量。
    pub variation: [f32; 6],
    pub note_count: u32,
    pub row_count: u32,
    pub avg_nps: f32,
    pub peak_nps: f32,
    pub chord_rate: f32,
    pub large_chord_rate: f32,
    pub unclassified: f32,
    pub algorithm_version: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManiaMmaCluster {
    pub pattern: String,
    pub specific_types: Vec<(String, f32)>,
    pub rating_multiplier: f32,
    pub bpm: i32,
    pub mixed: bool,
    pub amount: f32,
    pub importance: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManiaMmaBar {
    pub pattern: String,
    pub amount: f32,
    pub bpm: f32,
    pub relative: f32,
    pub specific_types: Vec<(String, f32)>,
}

impl ManiaMmaAnalysis {
    /// 绑定谱面身份，得到可持久化的记录。
    pub fn into_record(
        self,
        beatmap_id: u64,
        beatmapset_id: u64,
        game_mod: ManiaGameMod,
    ) -> ManiaMmaRecord {
        let dominant = self
            .report
            .clusters
            .first()
            .map(|cluster| cluster.pattern.to_owned())
            .unwrap_or_default();

        ManiaMmaRecord {
            beatmap_id,
            beatmapset_id,
            game_mod,
            key_count: self.key_count,
            mode_tag: self.report.mode_tag.to_owned(),
            category: self.report.category.clone(),
            dominant_pattern: dominant,
            ln_note_ratio: self.report.ln_percent as f32,
            hb_row_ratio: self.report.hb_row_ratio as f32,
            sv_amount: self.report.sv_amount as f32,
            duration_seconds: (self.report.duration / 1000.0) as f32,
            coverage: self.features.coverage.map(|value| value as f32),
            clusters: self
                .report
                .clusters
                .iter()
                .map(|cluster| ManiaMmaCluster {
                    pattern: cluster.pattern.to_owned(),
                    specific_types: cluster
                        .specific_types
                        .iter()
                        .map(|(name, ratio)| (name.clone(), *ratio as f32))
                        .collect(),
                    rating_multiplier: cluster.rating_multiplier as f32,
                    bpm: cluster.bpm as i32,
                    mixed: cluster.mixed,
                    amount: cluster.amount as f32,
                    importance: cluster.importance as f32,
                })
                .collect(),
            bars: self
                .features
                .bars
                .iter()
                .map(|bar| ManiaMmaBar {
                    pattern: bar.pattern.to_owned(),
                    amount: bar.amount as f32,
                    bpm: bar.bpm as f32,
                    relative: bar.relative as f32,
                    specific_types: bar
                        .specific_types
                        .iter()
                        .map(|(name, ratio)| (name.clone(), *ratio as f32))
                        .collect(),
                })
                .collect(),
            subtypes: self
                .features
                .subtypes
                .iter()
                .map(|(name, ratio)| (name.clone(), *ratio as f32))
                .collect(),
            intensity: self.features.intensity.map(|value| value as f32),
            temporal: self.features.temporal.map(|value| value as f32),
            variation: self.features.variation.map(|value| value as f32),
            note_count: self.features.note_count,
            row_count: self.features.row_count,
            avg_nps: self.features.avg_nps as f32,
            peak_nps: self.features.peak_nps as f32,
            chord_rate: self.features.chord_rate as f32,
            large_chord_rate: self.features.large_chord_rate as f32,
            unclassified: self.features.unclassified as f32,
            algorithm_version: MANIA_MMA_ALGORITHM_VERSION,
        }
    }
}

/// 记录格式版本；字段顺序变化必须提升。
pub const MANIA_MMA_ALGORITHM_VERSION: u32 = 1;

/// 便捷入口：直接得到绑定身份的记录。
pub fn analyze_record(
    text: &str,
    beatmap_id: u64,
    beatmapset_id: u64,
    game_mod: ManiaGameMod,
) -> Result<ManiaMmaRecord> {
    Ok(analyze(text, game_mod)?.into_record(beatmap_id, beatmapset_id, game_mod))
}
