// Dump this crate's primitives and pattern matches for one beatmap and clock rate.
// Usage: cargo run --release --example mma_dump -- <beatmap.osu> <NM|DT|HT> <out.json>
use std::fs;
use std::process::ExitCode;

use osu_difficulty_lab::ManiaGameMod;
use osu_difficulty_lab::mania_mma::{clustering, parser, patterns, primitives, rate, summary};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: mma_dump <beatmap.osu> <NM|DT|HT> <out.json>");
        return ExitCode::from(2);
    }
    let Some(game_mod) = ManiaGameMod::from_code(&args[2]) else {
        eprintln!("unknown mod");
        return ExitCode::from(2);
    };

    let text = fs::read_to_string(&args[1]).expect("beatmap");
    let transformed = rate::transform_rate(&text, game_mod.rate()).expect("transform");
    let chart = parser::parse_osu_mania(&transformed).expect("parse");

    let primitives: Vec<String> = primitives::calculate_primitives(&chart)
        .iter()
        .map(|row| {
            format!(
                "{{\"Index\":{},\"Time\":{},\"MsPerBeat\":{},\"BeatLength\":{},\"Notes\":{},\"Jacks\":{},\"Direction\":\"{:?}\",\"Roll\":{},\"LNHeads\":{:?},\"LNBodies\":{:?},\"LNTails\":{:?},\"NormalNotes\":{:?},\"RawNotes\":{:?}}}",
                row.index,
                row.time,
                row.ms_per_beat,
                row.beat_length,
                row.notes,
                row.jacks,
                row.direction,
                row.roll,
                row.ln_heads,
                row.ln_bodies,
                row.ln_tails,
                row.normal_notes,
                row.raw_notes
            )
        })
        .collect();

    let patterns: Vec<String> = patterns::find(&chart)
        .iter()
        .map(|pattern| {
            format!(
                "{{\"Pattern\":\"{}\",\"SpecificType\":{},\"Mixed\":{},\"Start\":{},\"End\":{},\"MsPerBeat\":{}}}",
                pattern.pattern,
                match pattern.specific_type {
                    Some(name) => format!("\"{name}\""),
                    None => "null".to_owned(),
                },
                pattern.mixed,
                pattern.start,
                pattern.end,
                pattern.ms_per_beat
            )
        })
        .collect();

    let report = summary::from_chart(&chart);
    // 直接用端口自己的聚类函数跑一遍，判断差异在模式列表还是聚类内部。
    let filtered: Vec<_> = patterns::find(&chart)
        .into_iter()
        .filter(|pattern| {
            report.mode_tag != "RC"
                || !matches!(pattern.pattern, "Coordination" | "Density" | "Wildcard")
        })
        .collect();
    let direct = clustering::specific_clusters(&filtered, report.mode_tag);
    let direct_debug: Vec<String> = direct
        .iter()
        .map(|cluster| {
            format!(
                "{{\"Pattern\":\"{}\",\"Mixed\":{},\"BPM\":{},\"Amount\":{}}}",
                cluster.pattern, cluster.mixed, cluster.bpm, cluster.amount
            )
        })
        .collect();
    let filtered_debug: Vec<String> = filtered
        .iter()
        .map(|pattern| {
            format!(
                "{{\"Pattern\":\"{}\",\"Mixed\":{},\"MsPerBeat\":{}}}",
                pattern.pattern, pattern.mixed, pattern.ms_per_beat
            )
        })
        .collect();
    // 与上游 assignClusters 相同的累计过程，用于定位 BPM 取整差异。
    let mut sums: Vec<(bool, &'static str, f64, f64, usize)> = Vec::new();
    for pattern in patterns::find(&chart) {
        if report.mode_tag == "RC"
            && matches!(pattern.pattern, "Coordination" | "Density" | "Wildcard")
        {
            continue;
        }
        if pattern.mixed {
            match sums
                .iter_mut()
                .find(|(mixed, name, _, _, _)| *mixed && *name == pattern.pattern)
            {
                Some((_, _, original, total, count)) => {
                    let _ = original;
                    *total += pattern.ms_per_beat;
                    *count += 1;
                }
                None => sums.push((
                    true,
                    pattern.pattern,
                    pattern.ms_per_beat,
                    pattern.ms_per_beat,
                    1,
                )),
            }
        } else {
            match sums.iter_mut().find(|(mixed, _, original, _, _)| {
                !*mixed && (original - pattern.ms_per_beat).abs() < 5.0
            }) {
                Some((_, _, _, total, count)) => {
                    *total += pattern.ms_per_beat;
                    *count += 1;
                }
                None => sums.push((
                    false,
                    pattern.pattern,
                    pattern.ms_per_beat,
                    pattern.ms_per_beat,
                    1,
                )),
            }
        }
    }
    let debug: Vec<String> = sums
        .iter()
        .map(|(mixed, name, original, total, count)| {
            format!(
                "{{\"Mixed\":{mixed},\"Pattern\":\"{name}\",\"Original\":{original},\"Sum\":{total},\"Count\":{count},\"Average\":{},\"Bpm\":{}}}",
                total / *count as f64,
                (60000.0 / (total / *count as f64)).round()
            )
        })
        .collect();
    let clusters: Vec<String> = report
        .clusters
        .iter()
        .map(|cluster| {
            let types: Vec<String> = cluster
                .specific_types
                .iter()
                .map(|(name, ratio)| format!("[\"{name}\",{ratio}]"))
                .collect();
            format!(
                "{{\"Pattern\":\"{}\",\"SpecificTypes\":[{}],\"RatingMultiplier\":{},\"BPM\":{},\"Mixed\":{},\"Amount\":{},\"Importance\":{}}}",
                cluster.pattern,
                types.join(","),
                cluster.rating_multiplier,
                cluster.bpm,
                cluster.mixed,
                cluster.amount,
                cluster.importance
            )
        })
        .collect();

    let json = format!(
        "{{\"primitives\":[{}],\"patterns\":[{}],\"assign\":[{}],\"direct\":[{}],\"filtered\":[{}],\"report\":{{\"Clusters\":[{}],\"Category\":\"{}\",\"ModeTag\":\"{}\",\"LNPercent\":{},\"HBRowRatio\":{},\"SVAmount\":{},\"Duration\":{}}}}}",
        primitives.join(","),
        patterns.join(","),
        debug.join(","),
        direct_debug.join(","),
        filtered_debug.join(","),
        clusters.join(","),
        report.category,
        report.mode_tag,
        report.ln_percent,
        report.hb_row_ratio,
        report.sv_amount,
        report.duration
    );
    fs::write(&args[3], json).expect("write dump");
    println!(
        "primitives={} patterns={} clusters={}",
        primitives.len(),
        patterns.len(),
        report.clusters.len()
    );
    ExitCode::SUCCESS
}
