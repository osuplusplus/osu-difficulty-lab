//! 键型分析的固定参数，对应上游 `js/patterns/config.js`。
//!
//! 这里的数值必须与上游保持一致；改动会让历史记录失去可比性，因此需要同时提升
//! [`MANIA_MMA_ALGORITHM_VERSION`](crate::MANIA_MMA_ALGORITHM_VERSION)。

/// 六大键型的评分倍率，对应 `CORE_RATING_MULTIPLIER`。
pub fn core_rating_multiplier(pattern: &str) -> f64 {
    match pattern {
        "Stream" => 1.0 / 3.0,
        "Chordstream" => 0.65,
        "Jacks" => 0.9,
        "Coordination" => 0.75,
        "Density" => 0.9,
        "Wildcard" => 1.0,
        _ => 1.0,
    }
}

/// RC 细分键型的基础倍率，对应 `RC_SUBTYPE_BASE`。
fn rc_subtype_base(name: &str) -> Option<f64> {
    Some(match name {
        "Rolls" | "Trills" | "Minitrills" => 1.0 / 3.0,
        "Handstream" | "Split Trill" | "Jumptrill" | "Jumpstream" | "Brackets"
        | "Double Stream" | "Dense Chordstream" | "Light Chordstream" | "Chord Rolls" => 0.65,
        "Longjacks" | "Quadstream" | "Gluts" | "Chordjacks" | "Minijacks" => 0.9,
        _ => return None,
    })
}

/// LN 细分键型的基础倍率，对应 `LN_SUBTYPE_BASE`。
fn ln_subtype_base(name: &str) -> Option<f64> {
    Some(match name {
        "Column Lock" => 1.5,
        "Release" => 0.73,
        "Shield" => 0.8,
        "JS Density" | "HS Density" | "DS Density" | "LCS Density" | "DCS Density" => 1.0,
        "Inverse" => 1.5,
        "Jacky WC" => 0.55,
        "Speedy WC" => 0.8,
        _ => return None,
    })
}

/// LN 模式的细分覆盖值，对应 `SUBTYPE_RATING_MULTIPLIER_BY_MODE.LN`。
///
/// 上游在 LN 模式下用这组值覆盖基础倍率（例如 Release 从 0.73 提到 1.0、
/// JS Density 从 1.0 降到 0.9），不能回落到基础表。
fn ln_mode_subtype(name: &str) -> Option<f64> {
    Some(match name {
        "Column Lock" => 1.5,
        "Release" => 1.0,
        "Shield" => 0.8,
        "JS Density" | "HS Density" | "DS Density" | "LCS Density" | "DCS Density" => 0.9,
        "Inverse" => 1.5,
        "Jacky WC" => 0.55,
        "Speedy WC" => 0.8,
        _ => return None,
    })
}

/// 按模式覆盖后的细分键型倍率，对应 `SUBTYPE_RATING_MULTIPLIER_BY_MODE`。
///
/// 返回 `None` 表示该名称不在覆盖表里，调用方应回落到核心倍率。
pub fn subtype_rating_multiplier(mode_tag: &str, name: &str) -> Option<f64> {
    match mode_tag {
        "RC" => rc_subtype_base(name).or_else(|| ln_subtype_base(name)),
        "LN" => ln_mode_subtype(name).or_else(|| rc_subtype_base(name)),
        "HB" => match name {
            "Column Lock" => Some(1.5),
            "Release" => Some(0.3),
            "Shield" => Some(0.8),
            "JS Density" | "HS Density" | "DS Density" | "LCS Density" | "DCS Density" => Some(0.9),
            "Inverse" => Some(0.0),
            "Jacky WC" => Some(0.65),
            "Speedy WC" => Some(0.45),
            _ => rc_subtype_base(name),
        },
        // Mix 与未知模式：上游默认走 Mix。
        _ => match name {
            "Column Lock" => Some(1.5),
            "Release" => Some(0.3),
            "Shield" => Some(0.8),
            "JS Density" | "HS Density" | "DS Density" | "LCS Density" | "DCS Density" => Some(0.9),
            "Inverse" => Some(0.0),
            "Jacky WC" => Some(0.45),
            "Speedy WC" => Some(0.45),
            _ => rc_subtype_base(name),
        },
    }
}

pub const RC_CORE_LN_SCALE: f64 = 0.3;
pub const RC_LN_CORE_SCALE: f64 = 0.0;
pub const RELEASE_WITH_DW_MULTIPLIER: f64 = 0.8;
pub const LN_MODE_LOW_THRESHOLD: f64 = 0.15;
pub const LN_MODE_HIGH_THRESHOLD: f64 = 0.9;
pub const HB_ROW_RATIO_THRESHOLD: f64 = 0.1;
pub const BPM_CLUSTER_THRESHOLD: f64 = 5.0;
pub const PATTERN_STABILITY_THRESHOLD: f64 = 5.0;
pub const IMPORTANT_CLUSTER_RATIO: f64 = 0.5;
pub const CATEGORY_JS_HS_SECONDARY_RATIO: f64 = 0.4;
pub const SV_AMOUNT_THRESHOLD: f64 = 2000.0;
pub const SV_SPEED_EPS: f64 = 0.05;
pub const SV_EXTREME_BPM_MIN: f64 = 20.0;
pub const SV_EXTREME_BPM_MAX: f64 = 450.0;
pub const SV_EXTREME_BPM_RATIO: f64 = 4.0;
pub const LONGJACK_VIBRO_RATIO_THRESHOLD: f64 = 0.6;
pub const LONGJACK_VIBRO_MIN_BPM: i64 = 180;
pub const CLUSTER_SPECIFIC_NAME_MIN_RATIO: f64 = 0.0;
pub const ENABLE_MULTI_LABEL_SAME_WINDOW: bool = true;
pub const COORDINATION_SPECIFIC_ORDER: [&str; 3] = ["Column Lock", "Shield", "Release"];
pub const DENSITY_SPECIFIC_ORDER: [&str; 6] = [
    "Inverse",
    "JS Density",
    "HS Density",
    "DS Density",
    "DCS Density",
    "LCS Density",
];
pub const WILDCARD_SPECIFIC_ORDER: [&str; 2] = ["Speedy WC", "Jacky WC"];
pub const JACKY_MIN_BPM: f64 = 90.0;
pub const SHIELD_MAX_BEAT_RATIO: f64 = 0.25;
pub const INVERSE_GAP_TOLERANCE_MS: f64 = 5.0;
pub const INVERSE_MIN_FILLED_LANES: usize = 3;
pub const RELEASE_SCAN_ROWS: usize = 4;
pub const RELEASE_MIN_TAIL_ROWS: usize = 4;
pub const RELEASE_ROLL_POINTS: usize = 2;
pub const RELEASE_FULL_MATCH_ROWS: usize = 5;
pub const JACKY_CONTEXT_WINDOW: usize = 6;
pub const JACKY_FALLBACK_MAX_MSPB: f64 = 185.0;

/// 六大键型，顺序与上游 `CORE_PATTERN_LIST` 一致。
pub const CORE_PATTERN_LIST: [&str; 6] = [
    "Stream",
    "Chordstream",
    "Jacks",
    "Coordination",
    "Density",
    "Wildcard",
];

/// 对应上游 `modeTagFromLnRatio`。
pub fn mode_tag_from_ln_ratio(ln_ratio: f64) -> &'static str {
    if !ln_ratio.is_finite() {
        return "Mix";
    }
    if ln_ratio <= LN_MODE_LOW_THRESHOLD {
        return "RC";
    }
    if ln_ratio >= LN_MODE_HIGH_THRESHOLD {
        return "LN";
    }
    "Mix"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 上游 `SUBTYPE_RATING_MULTIPLIER_BY_MODE` 里 LN 模式用独立的覆盖值，
    /// 例如 Release 由 0.73 提到 1.0、JS Density 由 1.0 降到 0.9。
    #[test]
    fn ln_mode_uses_its_own_overrides() {
        assert_eq!(subtype_rating_multiplier("LN", "Release"), Some(1.0));
        assert_eq!(subtype_rating_multiplier("LN", "JS Density"), Some(0.9));
        assert_eq!(subtype_rating_multiplier("LN", "HS Density"), Some(0.9));
        assert_eq!(subtype_rating_multiplier("LN", "DCS Density"), Some(0.9));
        assert_eq!(subtype_rating_multiplier("LN", "Inverse"), Some(1.5));
        assert_eq!(subtype_rating_multiplier("LN", "Column Lock"), Some(1.5));
        assert_eq!(subtype_rating_multiplier("LN", "Shield"), Some(0.8));
        assert_eq!(subtype_rating_multiplier("LN", "Jacky WC"), Some(0.55));
        assert_eq!(subtype_rating_multiplier("LN", "Speedy WC"), Some(0.8));
        // LN 模式仍然沿用 RC 的普通细分值。
        assert_eq!(subtype_rating_multiplier("LN", "Jumpstream"), Some(0.65));
        assert_eq!(subtype_rating_multiplier("LN", "Rolls"), Some(1.0 / 3.0));
    }

    #[test]
    fn other_modes_keep_the_upstream_tables() {
        // RC 模式把 LN_SUBTYPE_BASE 展开进基础表。
        assert_eq!(subtype_rating_multiplier("RC", "Release"), Some(0.73));
        assert_eq!(subtype_rating_multiplier("RC", "JS Density"), Some(1.0));
        assert_eq!(subtype_rating_multiplier("RC", "Shield"), Some(0.8));
        // HB / Mix 使用各自的覆盖值。
        assert_eq!(subtype_rating_multiplier("HB", "Release"), Some(0.3));
        assert_eq!(subtype_rating_multiplier("HB", "Inverse"), Some(0.0));
        assert_eq!(subtype_rating_multiplier("HB", "Jacky WC"), Some(0.65));
        assert_eq!(subtype_rating_multiplier("Mix", "Release"), Some(0.3));
        assert_eq!(subtype_rating_multiplier("Mix", "Inverse"), Some(0.0));
        assert_eq!(subtype_rating_multiplier("Mix", "Jacky WC"), Some(0.45));
        assert_eq!(subtype_rating_multiplier("Mix", "Speedy WC"), Some(0.45));
        // 未知模式回落到 Mix。
        assert_eq!(subtype_rating_multiplier("unknown", "Release"), Some(0.3));
    }

    #[test]
    fn mode_tags_follow_the_upstream_thresholds() {
        assert_eq!(mode_tag_from_ln_ratio(0.0), "RC");
        assert_eq!(mode_tag_from_ln_ratio(0.15), "RC");
        assert_eq!(mode_tag_from_ln_ratio(0.5), "Mix");
        assert_eq!(mode_tag_from_ln_ratio(0.9), "LN");
        assert_eq!(mode_tag_from_ln_ratio(f64::NAN), "Mix");
    }
}
