//! 供推荐使用的派生特征：六类覆盖率、细分键型、强度与时间结构。
//!
//! 覆盖率是「该类匹配区间的并集时长 ÷ 首尾音符间时长」，因此六类可以重叠、不要求和为 1。
//! 与上游显示一致：不做按最大值归一化，最长的一类不会自动变成满格。

use super::chart::{Chart, NoteType};
use super::config::CORE_PATTERN_LIST;
use super::patterns::{
    CORE_COORDINATION_NAME, CORE_DENSITY_NAME, CORE_WILDCARD_NAME, FoundPattern, find,
};
use super::summary::MmaReport;

/// 时间结构的统计窗口长度。
pub const WINDOW_MS: f64 = 4000.0;

const LN_CORE_PATTERNS: [&str; 3] = [
    CORE_COORDINATION_NAME,
    CORE_DENSITY_NAME,
    CORE_WILDCARD_NAME,
];

/// 合并后的键型条，对应 OPP 侧 `displayProjection` 的 bars。
#[derive(Debug, Clone)]
pub struct PatternBar {
    pub pattern: &'static str,
    pub amount: f64,
    pub bpm: f64,
    /// 相对最长条的比例，仅用于显示。
    pub relative: f64,
    pub specific_types: Vec<(String, f64)>,
}

/// 一次分析的全部派生特征。
#[derive(Debug, Clone)]
pub struct MmaFeatures {
    pub coverage: [f64; 6],
    pub subtypes: Vec<(String, f64)>,
    pub bars: Vec<PatternBar>,
    /// 平均 NPS、峰值 NPS（95 分位）、持续 NPS（中位）、最长持续段秒数。
    pub intensity: [f64; 4],
    /// 持续段占比、最长持续段占全谱比例、空窗比例。
    pub temporal: [f64; 3],
    /// 每个键型在相邻时间窗之间的平均变化量。
    pub variation: [f64; 6],
    pub note_count: u32,
    pub row_count: u32,
    pub avg_nps: f64,
    pub peak_nps: f64,
    pub chord_rate: f64,
    pub large_chord_rate: f64,
    pub unclassified: f64,
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

/// 对应 OPP 侧 `quantile`：升序排列后取 floor((n-1)q)。
fn quantile(values: &[f64], q: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let index = ((sorted.len() as f64 - 1.0) * q).floor() as usize;
    sorted[index.min(sorted.len() - 1)]
}

/// 对应 OPP 侧 `unionLength`：区间并集长度。
fn union_length(mut spans: Vec<(f64, f64)>) -> f64 {
    let mut total = 0.0;
    let mut end = f64::NEG_INFINITY;
    spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    for (start, stop) in spans {
        total += (stop - start.max(end)).max(0.0);
        end = end.max(stop);
    }
    total
}

/// 对应 OPP 侧 `displayProjection(report, false)`。
fn display_bars(report: &MmaReport) -> Vec<PatternBar> {
    let mut order: Vec<&'static str> = Vec::new();
    let mut amounts: Vec<f64> = Vec::new();
    let mut bpms: Vec<f64> = Vec::new();
    let mut specific: Vec<Vec<(String, f64)>> = Vec::new();

    for cluster in &report.clusters {
        let index = match order.iter().position(|name| *name == cluster.pattern) {
            Some(index) => index,
            None => {
                order.push(cluster.pattern);
                amounts.push(0.0);
                bpms.push(0.0);
                specific.push(Vec::new());
                order.len() - 1
            }
        };
        amounts[index] += cluster.amount;
        bpms[index] = bpms[index].max(cluster.bpm as f64);
        for (name, ratio) in &cluster.specific_types {
            let entry = specific[index]
                .iter_mut()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value);
            match entry {
                Some(value) => *value += ratio * cluster.amount,
                None => specific[index].push((name.clone(), ratio * cluster.amount)),
            }
        }
    }

    let max = amounts.iter().take(5).cloned().fold(1.0_f64, f64::max);

    order
        .iter()
        .enumerate()
        .map(|(index, pattern)| {
            let amount = amounts[index];
            let mut types: Vec<(String, f64)> = specific[index]
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        value / if amount == 0.0 { 1.0 } else { amount },
                    )
                })
                .collect();
            types.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            PatternBar {
                pattern,
                amount,
                bpm: bpms[index],
                relative: clamp01(amount / max),
                specific_types: types,
            }
        })
        .collect()
}

/// 由谱面与报告生成推荐特征。
pub fn derive(chart: &Chart, report: &MmaReport) -> MmaFeatures {
    let mut heads: Vec<(f64, usize)> = Vec::new();
    for row in &chart.notes {
        let notes = row
            .data
            .iter()
            .filter(|note| matches!(note, NoteType::Normal | NoteType::HoldHead))
            .count();
        if notes > 0 {
            heads.push((row.time, notes));
        }
    }

    let first_note = chart.first_note();
    let duration = (chart.last_note() - first_note).max(1.0);

    let window_count = (duration / WINDOW_MS).ceil().max(1.0) as usize;
    let mut window_starts = Vec::with_capacity(window_count);
    let mut window_ends = Vec::with_capacity(window_count);
    let mut window_notes = vec![0usize; window_count];
    let mut window_patterns = vec![[0.0_f64; 6]; window_count];
    for index in 0..window_count {
        let start = index as f64 * WINDOW_MS;
        window_starts.push(start);
        window_ends.push(duration.min((index as f64 + 1.0) * WINDOW_MS));
    }
    for (time, notes) in &heads {
        let raw = ((time - first_note) / WINDOW_MS).floor();
        let index = (raw.max(0.0) as usize).min(window_count - 1);
        window_notes[index] += notes;
    }

    let intervals: Vec<FoundPattern> = find(chart)
        .into_iter()
        .filter(|pattern| report.mode_tag != "RC" || !LN_CORE_PATTERNS.contains(&pattern.pattern))
        .collect();

    let mut coverage = [0.0_f64; 6];
    for (index, name) in CORE_PATTERN_LIST.iter().enumerate() {
        let spans: Vec<(f64, f64)> = intervals
            .iter()
            .filter(|pattern| pattern.pattern == *name)
            .map(|pattern| {
                (
                    (pattern.start - first_note).max(0.0),
                    (pattern.end - first_note).min(duration),
                )
            })
            .collect();
        coverage[index] = union_length(spans.clone()) / duration;

        for window in 0..window_count {
            let clipped: Vec<(f64, f64)> = spans
                .iter()
                .filter(|(start, end)| *start < window_ends[window] && *end > window_starts[window])
                .map(|(start, end)| {
                    (
                        start.max(window_starts[window]),
                        end.min(window_ends[window]),
                    )
                })
                .collect();
            let width = window_ends[window] - window_starts[window];
            window_patterns[window][index] = if width > 0.0 {
                union_length(clipped) / width
            } else {
                0.0
            };
        }
    }

    let mut subtype_names: Vec<&'static str> = Vec::new();
    for pattern in &intervals {
        if let Some(name) = pattern.specific_type
            && !subtype_names.contains(&name)
        {
            subtype_names.push(name);
        }
    }
    let subtypes: Vec<(String, f64)> = subtype_names
        .into_iter()
        .map(|name| {
            let spans: Vec<(f64, f64)> = intervals
                .iter()
                .filter(|pattern| pattern.specific_type == Some(name))
                .map(|pattern| {
                    (
                        (pattern.start - first_note).max(0.0),
                        (pattern.end - first_note).min(duration),
                    )
                })
                .collect();
            (name.to_owned(), union_length(spans) / duration)
        })
        .collect();

    let nps: Vec<f64> = (0..window_count)
        .map(|index| {
            let width = (window_ends[index] - window_starts[index]) / 1000.0;
            window_notes[index] as f64 / width.max(0.25)
        })
        .collect();

    let total_notes: usize = heads.iter().map(|(_, notes)| *notes).sum();
    let avg_nps = total_notes as f64 / (duration / 1000.0);
    let peak_nps = quantile(&nps, 0.95);
    let sustain_nps = quantile(&nps, 0.5);

    let mut run = 0.0_f64;
    let mut longest = 0.0_f64;
    for index in 0..window_count {
        if nps[index] >= peak_nps * 0.7 {
            run += (window_ends[index] - window_starts[index]) / 1000.0;
        } else {
            run = 0.0;
        }
        longest = longest.max(run);
    }

    let chord_rate = if heads.is_empty() {
        0.0
    } else {
        heads.iter().filter(|(_, notes)| *notes >= 2).count() as f64 / heads.len() as f64
    };
    let large_threshold = chart.keys.div_ceil(2);
    let large_chord_rate = if heads.is_empty() {
        0.0
    } else {
        heads
            .iter()
            .filter(|(_, notes)| *notes >= large_threshold)
            .count() as f64
            / heads.len() as f64
    };
    let break_density = if window_count == 0 {
        0.0
    } else {
        window_notes.iter().filter(|notes| **notes == 0).count() as f64 / window_count as f64
    };
    let peak_to_sustain_gap = if peak_nps > 0.0 {
        (peak_nps - sustain_nps) / peak_nps
    } else {
        0.0
    };
    let _ = peak_to_sustain_gap;

    let variation = if window_count < 2 {
        [0.0; 6]
    } else {
        let mut sums = [0.0_f64; 6];
        for index in 1..window_count {
            for pattern in 0..6 {
                sums[pattern] +=
                    (window_patterns[index][pattern] - window_patterns[index - 1][pattern]).abs();
            }
        }
        let divisor = (window_count - 1) as f64;
        sums.map(|value| value / divisor)
    };

    let unclassified = 1.0
        - union_length(
            intervals
                .iter()
                .map(|pattern| {
                    (
                        (pattern.start - first_note).max(0.0),
                        (pattern.end - first_note).min(duration),
                    )
                })
                .collect(),
        ) / duration;

    MmaFeatures {
        coverage,
        subtypes,
        bars: display_bars(report),
        intensity: [avg_nps, peak_nps, sustain_nps, longest],
        temporal: [
            clamp01(sustain_nps / if peak_nps == 0.0 { 1.0 } else { peak_nps }),
            clamp01(longest / (duration / 1000.0)),
            break_density,
        ],
        variation,
        note_count: total_notes as u32,
        row_count: heads.len() as u32,
        avg_nps,
        peak_nps,
        chord_rate,
        large_chord_rate,
        unclassified,
    }
}
