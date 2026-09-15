//! 主模式判定，对应上游 `js/patterns/categorise.js`。
//!
//! 主模式取「重要度最高的簇」的细分键型，而不是覆盖率最高的类别；`Tech` 表示该簇的
//! 命中区间内倍率不稳定（上游 `Mixed`）。

use super::clustering::PatternCluster;
use super::config::{CATEGORY_JS_HS_SECONDARY_RATIO, IMPORTANT_CLUSTER_RATIO};

/// 上游 `isHybridChart` 目前恒为 false，这里保留同样的语义。
fn is_hybrid_chart() -> bool {
    false
}

/// 对应上游 `categoriseChart`。
pub fn categorise_chart(ordered_clusters: &[PatternCluster]) -> String {
    let Some(first) = ordered_clusters.first() else {
        return "Uncategorised".to_owned();
    };

    let mut important = Vec::new();
    for cluster in ordered_clusters {
        if (cluster.importance / first.importance) > IMPORTANT_CLUSTER_RATIO {
            important.push(cluster);
        } else {
            break;
        }
    }

    let cluster1 = important[0];
    let hybrid = is_hybrid_chart();
    let tech = cluster1.mixed;

    let name = match cluster1.specific_types.first() {
        Some((name, ratio)) if *ratio > 0.05 => name.clone(),
        _ if cluster1.specific_types.len() >= 2
            && cluster1.specific_types[0].0 == "Jumpstream"
            && cluster1.specific_types[1].0 == "Handstream" =>
        {
            let primary = cluster1.specific_types[0].1;
            let secondary = cluster1.specific_types[1].1;
            if primary > 0.0 && (secondary / primary) > CATEGORY_JS_HS_SECONDARY_RATIO {
                "Jumpstream/Handstream".to_owned()
            } else {
                cluster1.pattern.to_owned()
            }
        }
        _ => cluster1.pattern.to_owned(),
    };

    format!(
        "{name}{}{}",
        if hybrid { " Hybrid" } else { "" },
        if tech { " Tech" } else { "" }
    )
}
