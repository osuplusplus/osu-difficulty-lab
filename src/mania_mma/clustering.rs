//! 键型聚类，对应上游 `js/patterns/clustering.js`。
//!
//! 聚类把逐窗口的匹配结果合并成「键型 + 混合标记 + BPM」的簇，并计算 Amount
//! （区间并集时长）与 Importance（Amount × 倍率 × BPM）。
//! `Importance` 在上游是 getter，这里在倍率修正完成后一次性算好。

use super::config::{
    BPM_CLUSTER_THRESHOLD, CLUSTER_SPECIFIC_NAME_MIN_RATIO, RELEASE_WITH_DW_MULTIPLIER,
};
use super::patterns::{
    CORE_DENSITY_NAME, CORE_WILDCARD_NAME, FoundPattern, resolve_rating_multiplier,
};

/// 一个键型簇，对应上游 `specificClusters` 产出的对象。
#[derive(Debug, Clone)]
pub struct PatternCluster {
    pub pattern: &'static str,
    /// 细分键型及其占比，已按占比降序排列。
    pub specific_types: Vec<(String, f64)>,
    pub rating_multiplier: f64,
    pub bpm: i64,
    pub mixed: bool,
    pub amount: f64,
    pub importance: f64,
}

impl PatternCluster {
    /// 对应上游 `format`。
    pub fn format(&self, rate: f64) -> String {
        let name = match self.specific_types.first() {
            Some((name, ratio)) if *ratio >= CLUSTER_SPECIFIC_NAME_MIN_RATIO => name.as_str(),
            _ => self.pattern,
        };
        let bpm = (self.bpm as f64 * rate).round() as i64;
        if self.mixed {
            format!("~{bpm}BPM Mixed {name}")
        } else {
            format!("{bpm}BPM {name}")
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ClusterBuilder {
    sum_ms: f64,
    original_ms_per_beat: f64,
    count: usize,
    bpm: i64,
}

impl ClusterBuilder {
    fn new(value: f64) -> Self {
        Self {
            sum_ms: value,
            original_ms_per_beat: value,
            count: 1,
            bpm: 0,
        }
    }

    fn add(&mut self, value: f64) {
        self.count += 1;
        self.sum_ms += value;
    }

    fn calculate(&mut self) {
        let average = self.sum_ms / self.count as f64;
        self.bpm = if average <= 0.0 {
            0
        } else {
            (60000.0 / average).round() as i64
        };
    }
}

/// 对应上游 `patternAmount`：区间并集长度。
fn pattern_amount(sorted_start_end: &[(f64, f64)]) -> f64 {
    let mut total = 0.0;
    let (mut current_start, mut current_end) = sorted_start_end[0];

    for (start, end) in sorted_start_end {
        if current_end < *end {
            total += current_end - current_start;
            current_start = *start;
            current_end = *end;
        } else {
            current_end = current_end.max(*end);
        }
    }

    total += current_end - current_start;
    total
}

/// 对应上游 `assignClusters`：返回每个匹配结果所属簇的最终 BPM。
fn assign_clusters(patterns: &[FoundPattern]) -> Vec<(FoundPattern, i64)> {
    // 混合簇按键型分别累计，非混合簇按倍率邻近合并。
    let mut bpms_non_mixed: Vec<ClusterBuilder> = Vec::new();
    let mut bpms_mixed: Vec<(&'static str, ClusterBuilder)> = Vec::new();
    let mut assignment: Vec<(bool, usize)> = Vec::with_capacity(patterns.len());

    for pattern in patterns {
        if pattern.mixed {
            // 命中已有簇才累加；新建时首个成员已经计入，不能再加一次。
            let index = match bpms_mixed
                .iter()
                .position(|(name, _)| *name == pattern.pattern)
            {
                Some(index) => {
                    bpms_mixed[index].1.add(pattern.ms_per_beat);
                    index
                }
                None => {
                    bpms_mixed.push((pattern.pattern, ClusterBuilder::new(pattern.ms_per_beat)));
                    bpms_mixed.len() - 1
                }
            };
            assignment.push((true, index));
        } else {
            let index = match bpms_non_mixed.iter().position(|cluster| {
                (cluster.original_ms_per_beat - pattern.ms_per_beat).abs() < BPM_CLUSTER_THRESHOLD
            }) {
                Some(index) => {
                    bpms_non_mixed[index].add(pattern.ms_per_beat);
                    index
                }
                None => {
                    bpms_non_mixed.push(ClusterBuilder::new(pattern.ms_per_beat));
                    bpms_non_mixed.len() - 1
                }
            };
            assignment.push((false, index));
        }
    }

    // 上游在分组前统一结算 BPM。
    for cluster in bpms_non_mixed.iter_mut() {
        cluster.calculate();
    }
    for (_, cluster) in bpms_mixed.iter_mut() {
        cluster.calculate();
    }

    patterns
        .iter()
        .zip(assignment)
        .map(|(pattern, (mixed, index))| {
            let bpm = if mixed {
                bpms_mixed[index].1.bpm
            } else {
                bpms_non_mixed[index].bpm
            };
            (pattern.clone(), bpm)
        })
        .collect()
}

/// 对应上游 `specificClusters`。
pub fn specific_clusters(patterns: &[FoundPattern], mode_tag: &str) -> Vec<PatternCluster> {
    let with_clusters = assign_clusters(patterns);

    // 分组键为「键型 + 混合标记 + BPM」，保持首次出现顺序。
    let mut group_keys: Vec<(&'static str, bool, i64)> = Vec::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (index, (pattern, bpm)) in with_clusters.iter().enumerate() {
        let key = (pattern.pattern, pattern.mixed, *bpm);
        match group_keys.iter().position(|existing| *existing == key) {
            Some(position) => groups[position].push(index),
            None => {
                group_keys.push(key);
                groups.push(vec![index]);
            }
        }
    }

    let mut out = Vec::with_capacity(groups.len());
    for (position, members) in groups.iter().enumerate() {
        let (pattern_name, mixed, bpm) = group_keys[position];

        let mut starts_ends: Vec<(f64, f64)> = members
            .iter()
            .map(|index| {
                let pattern = &with_clusters[*index].0;
                (pattern.start, pattern.end)
            })
            .collect();
        starts_ends.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let data_count = members.len();

        // 细分键型计数保持插入顺序，排序是稳定的。
        let mut counter: Vec<(String, usize)> = Vec::new();
        for index in members {
            if let Some(name) = with_clusters[*index].0.specific_type {
                match counter.iter_mut().find(|(key, _)| key == name) {
                    Some((_, count)) => *count += 1,
                    None => counter.push((name.to_owned(), 1)),
                }
            }
        }

        let mut specific_types: Vec<(String, f64)> = counter
            .into_iter()
            .map(|(name, count)| (name, count as f64 / data_count as f64))
            .collect();
        specific_types.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let dominant = specific_types.first().map(|(name, _)| name.clone());
        let rating_multiplier =
            resolve_rating_multiplier(pattern_name, dominant.as_deref(), mode_tag);
        let amount = if starts_ends.is_empty() {
            0.0
        } else {
            pattern_amount(&starts_ends)
        };

        out.push(PatternCluster {
            pattern: pattern_name,
            specific_types,
            rating_multiplier,
            bpm,
            mixed,
            amount,
            importance: 0.0,
        });
    }

    // Density / Wildcard 存在时压低 Release 的倍率。
    let has_dw = out.iter().any(|cluster| {
        cluster.pattern == CORE_DENSITY_NAME || cluster.pattern == CORE_WILDCARD_NAME
    });
    if has_dw && RELEASE_WITH_DW_MULTIPLIER != 1.0 {
        for cluster in out.iter_mut() {
            if cluster
                .specific_types
                .iter()
                .any(|(name, ratio)| name == "Release" && *ratio > 0.0)
            {
                cluster.rating_multiplier *= RELEASE_WITH_DW_MULTIPLIER;
            }
        }
    }

    for cluster in out.iter_mut() {
        cluster.importance = cluster.amount * cluster.rating_multiplier * cluster.bpm as f64;
    }

    out
}
