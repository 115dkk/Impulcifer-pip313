#![forbid(unsafe_code)]
//! Structure of the offline interactive report: six tabs in registry order, a
//! base64 Float32 payload per tab that decodes to exactly the model's series,
//! and no network reference of any kind.
use impulcifer_analysis::{
    html::{self, SUMMARY_TITLE},
    model::{self, Panel},
};
use impulcifer_dsp::{
    hrir::{Hrir, SpeakerIrs},
    ir::ImpulseResponse,
};
use serde_json::Value;

/// Deterministic decaying noise with an onset (xorshift, no RNG dependency).
fn response(n: usize, onset: usize, seed: u64, gain: f64) -> Vec<f64> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..n)
        .map(|i| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let noise = (state >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
            let decay = (-(i as f64) / 480.0).exp();
            gain * (if i == onset { 1.0 } else { 0.0 } + 0.1 * noise * decay)
        })
        .collect()
}

fn hrir() -> Hrir {
    let fs = 48000;
    let ir = |data| {
        Some(ImpulseResponse {
            data,
            fs,
            recording: None,
        })
    };
    Hrir {
        fs,
        // Deliberately out of canonical order: the report sorts FL, FR, FC.
        speakers: vec![
            SpeakerIrs {
                speaker: "FC".into(),
                left: ir(response(4800, 40, 3, 0.9)),
                right: ir(response(4800, 40, 4, 0.9)),
            },
            SpeakerIrs {
                speaker: "FL".into(),
                left: ir(response(4800, 40, 1, 1.0)),
                right: ir(response(4800, 52, 2, 0.6)),
            },
            SpeakerIrs {
                speaker: "FR".into(),
                left: ir(response(4800, 52, 5, 0.6)),
                right: ir(response(4800, 40, 6, 1.0)),
            },
        ],
    }
}

fn decode_base64(text: &str) -> Vec<u8> {
    let value = |c: u8| -> u32 {
        match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("invalid base64 byte {c}"),
        }
    };
    let bytes = text.trim().as_bytes();
    assert_eq!(bytes.len() % 4, 0, "base64 length");
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let pad = chunk.iter().rev().take_while(|c| **c == b'=').count();
        let mut n = 0;
        for (i, c) in chunk.iter().enumerate() {
            n |= if *c == b'=' { 0 } else { value(*c) } << (18 - 6 * i);
        }
        out.extend_from_slice(&[(n >> 16) as u8, (n >> 8) as u8, n as u8][..3 - pad]);
    }
    out
}

fn between<'a>(html: &'a str, open: &str) -> &'a str {
    let start = html.find(open).unwrap_or_else(|| panic!("missing {open}")) + open.len();
    let end = start + html[start..].find("</script>").unwrap();
    &html[start..end]
}

fn manifest(html: &str) -> Value {
    serde_json::from_str(between(
        html,
        "<script type=\"application/json\" id=\"report-manifest\">",
    ))
    .unwrap()
}

fn blob(html: &str, id: &str) -> Vec<f32> {
    let bytes = decode_base64(between(
        html,
        &format!("<script type=\"application/octet-stream\" id=\"{id}\">"),
    ));
    assert_eq!(bytes.len() % 4, 0);
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}

fn slice<'a>(data: &'a [f32], reference: &Value) -> &'a [f32] {
    let offset = reference[0].as_u64().unwrap() as usize;
    let len = reference[1].as_u64().unwrap() as usize;
    assert!(offset + len <= data.len(), "series outside its blob");
    &data[offset..offset + len]
}

fn as_f32(values: &[f64]) -> Vec<f32> {
    values.iter().map(|v| *v as f32).collect()
}

/// Nothing in the document may reach the network or another file.
fn assert_offline(html: &str) {
    for needle in [
        "http://",
        "https://",
        "//cdn",
        " src=",
        "<link",
        "@import",
        "url(",
        "fetch(",
        "XMLHttpRequest",
        "<iframe",
    ] {
        assert!(!html.contains(needle), "report references {needle:?}");
    }
}

#[test]
fn interactive_summary_is_offline_and_complete() {
    let hrir = hrir();
    let report = model::build_report(&hrir);
    let html = html::summary_html(&report).expect("every panel has data");
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains(&format!("<title>{SUMMARY_TITLE}</title>")));
    assert_eq!(SUMMARY_TITLE, "Interactive Plot Summary");
    assert_offline(&html);

    let m = manifest(&html);
    let tabs = m["tabs"].as_array().unwrap();
    let ids: Vec<_> = tabs.iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        [
            "interaural_overlay",
            "ild",
            "ipd",
            "iacc",
            "etc",
            "result_overview"
        ]
    );
    let titles: Vec<_> = tabs.iter().map(|t| t["title"].as_str().unwrap()).collect();
    let registry: Vec<_> = Panel::ALL.iter().map(|p| p.title()).collect();
    assert_eq!(titles, registry);

    for tab in tabs {
        let id = tab["id"].as_str().unwrap();
        let floats = tab["floats"].as_u64().unwrap() as usize;
        let charts = tab["charts"].as_array().unwrap();
        let expected_charts = if id == "result_overview" { 2 } else { 3 };
        assert_eq!(charts.len(), expected_charts, "{id} charts");
        if matches!(id, "ild" | "ipd") {
            // Bars travel in the manifest; there is no sample payload.
            assert!(tab["blob"].is_null() && floats == 0, "{id}");
            assert!(!html.contains(&format!("id=\"blob-{id}\"")));
            for chart in charts {
                assert_eq!(chart["bars"].as_array().unwrap().len(), 8, "{id} bands");
            }
            continue;
        }
        let data = blob(&html, tab["blob"].as_str().unwrap());
        assert_eq!(data.len(), floats, "{id}: decoded payload length");
        let mut covered = 0;
        for chart in charts {
            for series in chart["series"].as_array().unwrap() {
                let y = slice(&data, &series["y"]);
                assert!(!y.is_empty());
                covered += y.len();
                if let Some(x) = series.get("x").filter(|x| !x.is_null()) {
                    assert_eq!(slice(&data, x).len(), y.len(), "{id}: x and y lengths");
                    if id == "iacc" {
                        covered += y.len();
                    }
                }
            }
        }
        if id == "result_overview" {
            // One shared frequency grid plus five curves.
            let n = report.overview.as_ref().unwrap().frequency.len();
            assert_eq!(covered + n, floats);
        } else {
            assert_eq!(covered, floats, "{id}: every float belongs to one series");
        }
    }

    // Canonical order and the 2.x chart titles.
    let chart_titles = |id: &str| -> Vec<String> {
        tabs.iter().find(|t| t["id"] == id).unwrap()["charts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["title"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        chart_titles("interaural_overlay"),
        ["FL", "FR", "FC"].map(|s| format!("Interaural Impulse Response - {s}"))
    );
    assert_eq!(
        chart_titles("ild"),
        ["FL", "FR", "FC"].map(|s| format!("ILD - {s}"))
    );
    assert_eq!(chart_titles("etc")[0], "Energy Decay Curve (EDC) - FL");
    assert_eq!(
        chart_titles("result_overview")[0],
        "Overall Smoothed Frequency Response"
    );

    // The payload is the model's data, rounded to f32 once.
    let overlay = &tabs[0]["charts"][0];
    let data = blob(&html, "blob-interaural_overlay");
    let fl = &report.overlay[0];
    assert_eq!(fl.speaker, "FL");
    assert_eq!(slice(&data, &overlay["series"][0]["y"]), as_f32(fl.left));
    assert_eq!(slice(&data, &overlay["series"][1]["y"]), as_f32(fl.right));
    assert_eq!(overlay["series"][0]["start"], -(fl.origin() as f64));
    let edc = blob(&html, "blob-etc");
    let chart = &tabs[4]["charts"][0];
    assert_eq!(
        slice(&edc, &chart["series"][1]["y"]),
        as_f32(&report.edc[0].curves[1].1)
    );
    let overview = blob(&html, "blob-result_overview");
    let o = report.overview.as_ref().unwrap();
    let series = &tabs[5]["charts"][1]["series"][0];
    assert_eq!(slice(&overview, &series["x"]), as_f32(&o.frequency));
    assert_eq!(slice(&overview, &series["y"]), as_f32(&o.difference));
}

#[test]
fn analysis_pages_hold_one_panel_each() {
    let hrir = hrir();
    let report = model::build_report(&hrir);
    for panel in Panel::ALL {
        let page = html::panel_html(&report, panel).unwrap();
        assert!(page.contains(&format!("<title>{}</title>", panel.title())));
        assert_offline(&page);
        let tabs = manifest(&page)["tabs"].as_array().unwrap().clone();
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0]["id"], panel.name());
    }
    let names: Vec<_> = Panel::ALL
        .iter()
        .filter(|p| p.save_individually())
        .map(|p| p.name())
        .collect();
    assert_eq!(names, ["ild", "ipd", "iacc", "etc"]);
}

#[test]
fn reports_without_data_produce_no_document() {
    let empty = Hrir {
        fs: 48000,
        speakers: Vec::new(),
    };
    let report = model::build_report(&empty);
    assert!(report.is_empty());
    assert!(html::summary_html(&report).is_none());
    assert!(
        Panel::ALL
            .iter()
            .all(|p| html::panel_html(&report, *p).is_none())
    );

    // Only an EDC: the summary has exactly that tab.
    let lonely = Hrir {
        fs: 48000,
        speakers: vec![SpeakerIrs {
            speaker: "FC".into(),
            left: Some(ImpulseResponse {
                data: response(480, 5, 9, 1.0),
                fs: 48000,
                recording: None,
            }),
            right: None,
        }],
    };
    let report = model::build_report(&lonely);
    let html = html::summary_html(&report).unwrap();
    let tabs = manifest(&html)["tabs"].as_array().unwrap().clone();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0]["id"], "etc");
}

#[test]
fn renderer_assets_are_embedded_verbatim() {
    let hrir = hrir();
    let html = html::summary_html(&model::build_report(&hrir)).unwrap();
    let js = include_str!("../assets/report.js");
    let css = include_str!("../assets/report.css");
    assert!(html.contains(js) && html.contains(css));
    assert!(js.starts_with("// @ts-check"));
    // The renderer's contracts: lazy per-tab decoding, one frame per redraw,
    // off-screen skipping, size tracking and column decimation.
    for needle in [
        "requestAnimationFrame",
        "IntersectionObserver",
        "ResizeObserver",
        "devicePixelRatio",
        "decimate(",
        "loadBlob(entry.spec.blob)",
        "prefers-color-scheme",
    ] {
        assert!(js.contains(needle), "renderer lost {needle}");
    }
    assert!(css.contains("prefers-color-scheme: dark"));
}
