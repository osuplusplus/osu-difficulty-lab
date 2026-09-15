//! osu!mania 键型记录的回归测试。
//!
//! 合成谱面来自 `tests/fixtures/mma/fixture-4k.osu`，期望值是固定版本
//! osumania_map_analyser 对同一份谱面的输出。夹具是自造的，不包含任何真实谱面内容。

use std::path::PathBuf;

use osu_difficulty_lab::{
    ManiaAnalyzer, ManiaFeatureStore, ManiaGameMod, ManiaMmaAnalysis, analyze_mania_mma,
};
use serde_json::Value;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mma")
        .join(name)
}

fn fixture_text() -> String {
    std::fs::read_to_string(fixture("fixture-4k.osu")).expect("fixture beatmap")
}

fn expected() -> Value {
    let raw =
        std::fs::read_to_string(fixture("fixture-4k.expected.json")).expect("expected report");
    serde_json::from_str(&raw).expect("expected report json")
}

fn assert_close(field: &str, expected: f64, actual: f64) {
    let tolerance = 1e-6_f64.max(expected.abs() * 1e-9);
    assert!(
        (expected - actual).abs() <= tolerance,
        "{field}: expected {expected} but got {actual}"
    );
}

fn assert_matches_reference(analysis: &ManiaMmaAnalysis, expected: &Value) {
    let mode_tag = expected["ModeTag"].as_str().expect("mode tag");
    assert_eq!(analysis.report.mode_tag, mode_tag, "ModeTag");
    assert_eq!(
        analysis.report.category,
        expected["Category"].as_str().expect("category"),
        "Category"
    );
    assert_close(
        "LNPercent",
        expected["LNPercent"].as_f64().expect("ln percent"),
        analysis.report.ln_percent,
    );
    assert_close(
        "HBRowRatio",
        expected["HBRowRatio"].as_f64().expect("hb ratio"),
        analysis.report.hb_row_ratio,
    );
    assert_close(
        "SVAmount",
        expected["SVAmount"].as_f64().expect("sv amount"),
        analysis.report.sv_amount,
    );
    assert_close(
        "Duration",
        expected["Duration"].as_f64().expect("duration"),
        analysis.report.duration,
    );

    let clusters = expected["Clusters"].as_array().expect("clusters");
    assert_eq!(
        clusters.len(),
        analysis.report.clusters.len(),
        "cluster count"
    );
    for (index, cluster) in clusters.iter().enumerate() {
        let actual = &analysis.report.clusters[index];
        assert_eq!(
            actual.pattern,
            cluster["Pattern"].as_str().expect("pattern"),
            "Clusters[{index}].Pattern"
        );
        assert_eq!(
            actual.mixed,
            cluster["Mixed"].as_bool().expect("mixed"),
            "Clusters[{index}].Mixed"
        );
        assert_close(
            &format!("Clusters[{index}].Amount"),
            cluster["Amount"].as_f64().expect("amount"),
            actual.amount,
        );
        assert_close(
            &format!("Clusters[{index}].Importance"),
            cluster["Importance"].as_f64().expect("importance"),
            actual.importance,
        );
        assert_close(
            &format!("Clusters[{index}].RatingMultiplier"),
            cluster["RatingMultiplier"].as_f64().expect("multiplier"),
            actual.rating_multiplier,
        );
        assert_close(
            &format!("Clusters[{index}].BPM"),
            cluster["BPM"].as_f64().expect("bpm"),
            actual.bpm as f64,
        );

        let types = cluster["SpecificTypes"].as_array().expect("specific types");
        assert_eq!(
            types.len(),
            actual.specific_types.len(),
            "Clusters[{index}].SpecificTypes length"
        );
        for (type_index, entry) in types.iter().enumerate() {
            assert_eq!(
                actual.specific_types[type_index].0,
                entry[0].as_str().expect("specific name"),
                "Clusters[{index}].SpecificTypes[{type_index}]"
            );
            assert_close(
                &format!("Clusters[{index}].SpecificTypes[{type_index}].ratio"),
                entry[1].as_f64().expect("specific ratio"),
                actual.specific_types[type_index].1,
            );
        }
    }
}

#[test]
fn key_patterns_match_the_pinned_reference_for_every_clock_rate() {
    let text = fixture_text();
    let expected = expected();

    for (key, game_mod) in [
        ("NM", ManiaGameMod::Nm),
        ("DT", ManiaGameMod::Dt),
        ("HT", ManiaGameMod::Ht),
    ] {
        let analysis = analyze_mania_mma(&text, game_mod).expect("analysis");
        assert_matches_reference(&analysis, &expected[key]);
    }
}

#[test]
fn clock_rates_are_analyzed_separately() {
    let text = fixture_text();
    let no_mod = analyze_mania_mma(&text, ManiaGameMod::Nm).expect("NM");
    let double_time = analyze_mania_mma(&text, ManiaGameMod::Dt).expect("DT");
    let half_time = analyze_mania_mma(&text, ManiaGameMod::Ht).expect("HT");

    // 倍率改变的是实际密度与 BPM，不是把 NoMod 特征乘一个系数。
    assert!(double_time.features.peak_nps > no_mod.features.peak_nps);
    assert!(half_time.features.peak_nps < no_mod.features.peak_nps);
    assert!((no_mod.report.duration - 10250.0).abs() < 1.0);
    assert!((double_time.report.duration - 6833.0).abs() < 1.0);
    assert!((half_time.report.duration - 13666.0).abs() < 1.0);

    // 覆盖率是并集占比，可以重叠，不要求和为 1。
    let total: f64 = no_mod.features.coverage.iter().sum();
    assert!(total > 0.0);
    assert!(
        no_mod
            .features
            .coverage
            .iter()
            .all(|value| (0.0..=1.0).contains(value))
    );
}

#[test]
fn key_pattern_records_round_trip_through_the_store() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut store = ManiaFeatureStore::open(temp.path()).expect("store");

    let bytes = std::fs::read(fixture("fixture-4k.osu")).expect("fixture bytes");
    let (metadata, raw) = ManiaAnalyzer::new()
        .analyze_bytes(&bytes)
        .expect("raw analysis");
    assert!(store.append_raw(&metadata, &raw).expect("append raw"));

    let text = fixture_text();
    for game_mod in ManiaGameMod::ALL {
        let record = analyze_mania_mma(&text, game_mod)
            .expect("analysis")
            .into_record(metadata.beatmap_id, metadata.beatmapset_id, game_mod);
        assert!(
            store
                .append_mma(&metadata.checksum, &record)
                .expect("append"),
            "first append stores the record"
        );
        assert!(
            !store
                .append_mma(&metadata.checksum, &record)
                .expect("append"),
            "unchanged checksum is skipped"
        );
    }

    assert_eq!(store.mma_record_count().expect("count"), 3);
    assert_eq!(
        store
            .mma_record_count_for(ManiaGameMod::Nm)
            .expect("count nm"),
        1
    );
    assert_eq!(store.mma_records().expect("records").len(), 3);

    let no_mod = store
        .mma_for_id(metadata.beatmap_id, ManiaGameMod::Nm)
        .expect("nm record");
    assert_eq!(no_mod.category, "Coordination");
    assert_eq!(no_mod.key_count, 4);
    assert_eq!(no_mod.game_mod, ManiaGameMod::Nm);
    assert_eq!(no_mod.mode_tag, "Mix");
    assert_eq!(no_mod.coverage.len(), 6);
    assert!(!no_mod.clusters.is_empty());

    let double_time = store
        .mma_for_id(metadata.beatmap_id, ManiaGameMod::Dt)
        .expect("dt record");
    assert_ne!(no_mod.duration_seconds, double_time.duration_seconds);
}
