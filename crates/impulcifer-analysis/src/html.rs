//! Self-contained interactive HTML reports (replace the 2.x Bokeh layouts).
//!
//! One offline document: inline CSS, an inline vanilla-JS canvas renderer
//! (`assets/report.js`), a small JSON manifest and one base64 blob of
//! little-endian `f32` samples per tab. Nothing is fetched: no CDN, fonts or
//! scripts from the network. The renderer decodes a tab's blob when the tab is
//! first opened and draws with per-pixel-column decimation, so file size and
//! interaction cost stay bounded (docs/rust/PLOTS.md, "Interactive report").

use crate::model::{BandChart, EDC_RANGE_DB, EDC_RANGE_MS, Ear, OVERLAY_RANGE_MS, Panel, Report};
use serde_json::{Value, json};

/// 2.x `bokeh_output_file(..., title="Interactive Plot Summary")`.
pub const SUMMARY_TITLE: &str = "Interactive Plot Summary";

const CSS: &str = include_str!("../assets/report.css");
const JS: &str = include_str!("../assets/report.js");

/// Float32 payload of one tab; series refer to `[offset, length]` in floats.
#[derive(Default)]
struct Blob {
    floats: Vec<f32>,
}

impl Blob {
    fn push<I: IntoIterator<Item = f64>>(&mut self, values: I) -> Value {
        let offset = self.floats.len();
        // Display precision: f32 keeps ~7 significant digits (the model stays f64).
        self.floats.extend(values.into_iter().map(|v| v as f32));
        json!([offset, self.floats.len() - offset])
    }
}

struct Tab {
    panel: Panel,
    note: &'static str,
    charts: Vec<Value>,
    blob: Blob,
}

impl Tab {
    fn new(panel: Panel, note: &'static str) -> Self {
        Self {
            panel,
            note,
            charts: Vec::new(),
            blob: Blob::default(),
        }
    }
}

fn ear_color(ear: Ear) -> &'static str {
    match ear {
        Ear::Left => "left",
        Ear::Right => "right",
    }
}

/// A line series whose x is `(i + start) * step`.
fn even_series(
    blob: &mut Blob,
    name: &str,
    ear: Ear,
    data: &[f64],
    start: f64,
    step: f64,
) -> Value {
    json!({
        "name": name,
        "color": ear_color(ear),
        "dash": ear == Ear::Right,
        "width": 1.5,
        "y": blob.push(data.iter().copied()),
        "start": start,
        "step": step,
    })
}

fn signed_ms(value: f64) -> String {
    let text = format!("{value:+.2}");
    if text == "-0.00" {
        "+0.00".into()
    } else {
        text
    }
}

fn overlay_tab(report: &Report) -> Tab {
    let mut tab = Tab::new(
        Panel::InterauralOverlay,
        "Left and right impulse response of each speaker on one time axis; 0 ms is the earlier of the two peaks. Right is dashed.",
    );
    for chart in &report.overlay {
        let step = 1000.0 / f64::from(chart.fs);
        let start = -(chart.origin() as f64);
        let series = vec![
            even_series(
                &mut tab.blob,
                Ear::Left.label(),
                Ear::Left,
                chart.left,
                start,
                step,
            ),
            even_series(
                &mut tab.blob,
                Ear::Right.label(),
                Ear::Right,
                chart.right,
                start,
                step,
            ),
        ];
        let delay =
            (chart.right_peak as f64 - chart.left_peak as f64) / f64::from(chart.fs) * 1000.0;
        tab.charts.push(json!({
            "kind": "line",
            "title": format!("Interaural Impulse Response - {}", chart.speaker),
            "subtitle": format!("Right-ear peak {} ms relative to the left", signed_ms(delay)),
            "x": {"label": "Time (ms relative to peak)", "unit": "ms", "view": [OVERLAY_RANGE_MS.0, OVERLAY_RANGE_MS.1], "digits": 2},
            "y": {"label": "Amplitude", "sig": 4},
            "zero": true,
            "series": series,
            "points": [
                {"x": chart.time_ms(chart.left_peak), "y": chart.left[chart.left_peak], "color": "left"},
                {"x": chart.time_ms(chart.right_peak), "y": chart.right[chart.right_peak], "color": "right"},
            ],
        }));
    }
    tab
}

/// Category label under a bar: the octave centre (`lower * sqrt 2`), e.g. `1k`.
fn centre_label(lower: f64) -> String {
    let centre = (lower * std::f64::consts::SQRT_2).round();
    if centre >= 1000.0 {
        let k = centre / 1000.0;
        if k.fract() == 0.0 {
            format!("{k:.0}k")
        } else {
            format!("{k}k")
        }
    } else {
        format!("{centre:.0}")
    }
}

fn band_tab(charts: &[BandChart], panel: Panel) -> Tab {
    let ild = panel == Panel::Ild;
    let mut tab = Tab::new(
        panel,
        if ild {
            "Interaural level difference per octave band, 10·log10(P_left / P_right). Positive (blue) means the left ear is louder."
        } else {
            "Interaural phase difference per octave band in degrees, left minus right: the angle of the band's summed cross spectrum."
        },
    );
    for chart in charts {
        let bars: Vec<Value> = chart
            .bands
            .iter()
            .zip(&chart.labels)
            .zip(&chart.values)
            .map(|((band, detail), value)| {
                json!({"label": centre_label(band.0), "detail": detail, "value": value})
            })
            .collect();
        let mut spec = json!({
            "kind": "bar",
            "title": format!("{} - {}", if ild { "ILD" } else { "IPD" }, chart.speaker),
            "size": "short",
            "x": {"label": "Octave band centre (Hz)"},
            "bars": bars,
        });
        spec["y"] = if ild {
            json!({"label": "ILD (dB, Left/Right)", "unit": "dB", "digits": 1})
        } else {
            json!({"label": "IPD (Degrees, Left - Right)", "unit": "deg", "digits": 1,
                   "view": [-180, 180], "ticks": [-180, -135, -90, -45, 0, 45, 90, 135, 180]})
        };
        tab.charts.push(spec);
    }
    tab
}

fn iacc_tab(report: &Report) -> Tab {
    let mut tab = Tab::new(
        Panel::Iacc,
        "Normalized interaural cross-correlation within ±1 ms (ISO 3382-1). The legend gives the IACC, max |IACF|, and its lag; a negative lag means the right ear is later.",
    );
    for chart in &report.iacc {
        let result = &chart.result;
        let peak = result
            .lags_ms
            .iter()
            .position(|lag| *lag == result.tau_ms)
            .unwrap_or(0);
        let series = json!([{
            "name": chart.legend(),
            "color": "diff",
            "width": 1.75,
            "x": tab.blob.push(result.lags_ms.iter().copied()),
            "y": tab.blob.push(result.iacf.iter().copied()),
        }]);
        tab.charts.push(json!({
            "kind": "line",
            "title": format!("IACC - {}", chart.speaker),
            "size": "short",
            "x": {"label": "Interaural Delay (ms)", "unit": "ms", "digits": 3,
                  "view": [-chart.max_delay_ms * 1.1, chart.max_delay_ms * 1.1]},
            "y": {"label": "Cross-Correlation Coefficient", "digits": 3},
            "zero": true,
            "series": series,
            "points": [{"x": result.lags_ms[peak], "y": result.iacf[peak], "color": "diff"}],
        }));
    }
    tab
}

fn edc_tab(report: &Report) -> Tab {
    let mut tab = Tab::new(
        Panel::Edc,
        "Schroeder energy decay curve of each ear: the energy still to come after each instant, in dB relative to the total.",
    );
    for chart in &report.edc {
        let step = 1000.0 / f64::from(chart.fs);
        let series: Vec<Value> = chart
            .curves
            .iter()
            .map(|(ear, curve)| even_series(&mut tab.blob, ear.label(), *ear, curve, 0.0, step))
            .collect();
        tab.charts.push(json!({
            "kind": "line",
            "title": format!("Energy Decay Curve (EDC) - {}", chart.speaker),
            "x": {"label": "Time (ms)", "unit": "ms", "view": [EDC_RANGE_MS.0, EDC_RANGE_MS.1], "digits": 2},
            "y": {"label": "Decay (dB re total energy)", "unit": "dB", "view": [EDC_RANGE_DB.0, EDC_RANGE_DB.1], "digits": 2},
            "series": series,
        }));
    }
    tab
}

fn overview_tab(report: &Report) -> Tab {
    let mut tab = Tab::new(
        Panel::ResultOverview,
        "Left and right ears summed over all speakers: raw and smoothed (1/3 octave, 1/5 above 20 kHz) response, and the smoothed left minus right difference. The two charts zoom together.",
    );
    let Some(o) = &report.overview else {
        return tab;
    };
    let frequency = tab.blob.push(o.frequency.iter().copied());
    let line = |blob: &mut Blob, name: &str, color: &str, values: &[f64], raw: bool, dash: bool| {
        json!({
            "name": name,
            "color": color,
            "alpha": if raw { 0.35 } else { 1.0 },
            "width": if raw { 1.0 } else { 2.0 },
            "dash": dash,
            "hover": !raw,
            "x": frequency,
            "y": blob.push(values.iter().copied()),
        })
    };
    let series = vec![
        line(&mut tab.blob, "Left Raw", "left", &o.left_raw, true, false),
        line(
            &mut tab.blob,
            "Right Raw",
            "right",
            &o.right_raw,
            true,
            false,
        ),
        line(
            &mut tab.blob,
            "Left Smoothed",
            "left",
            &o.left_smoothed,
            false,
            false,
        ),
        line(
            &mut tab.blob,
            "Right Smoothed",
            "right",
            &o.right_smoothed,
            false,
            true,
        ),
    ];
    let difference = vec![json!({
        "name": "Difference (L-R)",
        "color": "diff",
        "width": 1.75,
        "fill": true,
        "x": frequency,
        "y": tab.blob.push(o.difference.iter().copied()),
    })];
    let x = json!({"label": "Frequency (Hz)", "unit": "Hz", "log": true, "view": [20, 20000], "digits": 1});
    tab.charts.push(json!({
        "kind": "line",
        "title": "Overall Smoothed Frequency Response",
        "wide": true,
        "size": "tall",
        "link": "result-overview",
        "x": x,
        "y": {"label": "Amplitude (dB)", "unit": "dB", "digits": 2},
        "series": series,
    }));
    tab.charts.push(json!({
        "kind": "line",
        "title": "Difference (L-R), smoothed",
        "subtitle": "Blue: left louder, red: right louder; dashed guides at ±3 dB.",
        "wide": true,
        "link": "result-overview",
        "x": x,
        "y": {"label": "Difference (dB)", "unit": "dB", "digits": 2},
        "zero": true,
        "guides": [-3.0, 3.0],
        "series": difference,
    }));
    tab
}

fn tab_for(report: &Report, panel: Panel) -> Option<Tab> {
    if !report.has(panel) {
        return None;
    }
    Some(match panel {
        Panel::InterauralOverlay => overlay_tab(report),
        Panel::Ild => band_tab(&report.ild, Panel::Ild),
        Panel::Ipd => band_tab(&report.ipd, Panel::Ipd),
        Panel::Iacc => iacc_tab(report),
        Panel::Edc => edc_tab(report),
        Panel::ResultOverview => overview_tab(report),
    })
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding, straight into `out`.
fn base64_into(bytes: &[u8], out: &mut String) {
    out.reserve(bytes.len().div_ceil(3) * 4);
    let mut chunks = bytes.chunks_exact(3);
    for c in &mut chunks {
        let n = u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]);
        for shift in [18, 12, 6, 0] {
            out.push(BASE64[(n >> shift & 63) as usize] as char);
        }
    }
    match *chunks.remainder() {
        [a] => {
            let n = u32::from(a) << 16;
            out.push(BASE64[(n >> 18 & 63) as usize] as char);
            out.push(BASE64[(n >> 12 & 63) as usize] as char);
            out.push_str("==");
        }
        [a, b] => {
            let n = u32::from(a) << 16 | u32::from(b) << 8;
            out.push(BASE64[(n >> 18 & 63) as usize] as char);
            out.push(BASE64[(n >> 12 & 63) as usize] as char);
            out.push(BASE64[(n >> 6 & 63) as usize] as char);
            out.push('=');
        }
        _ => (),
    }
}

fn escape_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// JSON that can sit inside `<script>`: no `<`, `>` or `&` survives unescaped.
fn script_json(value: &Value) -> String {
    value
        .to_string()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

fn document(title: &str, subtitle: &str, tabs: Vec<Tab>) -> String {
    let manifest_tabs: Vec<Value> = tabs
        .iter()
        .map(|tab| {
            let has_data = !tab.blob.floats.is_empty();
            json!({
                "id": tab.panel.name(),
                "title": tab.panel.title(),
                "note": tab.note,
                "blob": if has_data { Value::from(format!("blob-{}", tab.panel.name())) } else { Value::Null },
                "floats": tab.blob.floats.len(),
                "charts": tab.charts,
            })
        })
        .collect();
    let manifest = json!({
        "title": title,
        "subtitle": subtitle,
        "generator": format!("Impulcifer {}", env!("CARGO_PKG_VERSION")),
        "tabs": manifest_tabs,
    });
    let payload: usize = tabs.iter().map(|t| t.blob.floats.len() * 4).sum();
    let mut html =
        String::with_capacity(CSS.len() + JS.len() + payload.div_ceil(3) * 4 + 16 * 1024);
    html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<meta name=\"color-scheme\" content=\"light dark\">\n");
    html.push_str(&format!("<title>{}</title>\n<style>\n", escape_text(title)));
    html.push_str(CSS);
    html.push_str("</style>\n</head>\n<body>\n<noscript>This report draws its charts with JavaScript; enable it to see them.</noscript>\n");
    html.push_str("<script type=\"application/json\" id=\"report-manifest\">");
    html.push_str(&script_json(&manifest));
    html.push_str("</script>\n");
    let mut bytes = Vec::new();
    for tab in &tabs {
        if tab.blob.floats.is_empty() {
            continue;
        }
        bytes.clear();
        bytes.reserve(tab.blob.floats.len() * 4);
        for v in &tab.blob.floats {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        html.push_str(&format!(
            "<script type=\"application/octet-stream\" id=\"blob-{}\">",
            tab.panel.name()
        ));
        base64_into(&bytes, &mut html);
        html.push_str("</script>\n");
    }
    html.push_str("<script>\n");
    html.push_str(JS);
    html.push_str("</script>\n</body>\n</html>\n");
    html
}

fn subtitle(report: &Report) -> String {
    format!(
        "{} speaker{} · {} Hz",
        report.speakers,
        if report.speakers == 1 { "" } else { "s" },
        report.fs
    )
}

/// The 2.x `interactive_plots/interactive_summary.html`: one tab per panel that
/// has data, in registry order. `None` when no panel has data (2.x then logs
/// `cli_warning_no_interactive` and writes nothing).
pub fn summary_html(report: &Report) -> Option<String> {
    let tabs: Vec<Tab> = Panel::ALL
        .iter()
        .filter_map(|panel| tab_for(report, *panel))
        .collect();
    (!tabs.is_empty()).then(|| document(SUMMARY_TITLE, &subtitle(report), tabs))
}

/// One panel as its own document, the 2.x `plots/<name>/<name>_analysis.html`
/// (`--plot`), titled with the registry title. `None` when the panel is empty.
pub fn panel_html(report: &Report, panel: Panel) -> Option<String> {
    tab_for(report, panel).map(|tab| document(panel.title(), &subtitle(report), vec![tab]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            let mut out = String::new();
            base64_into(input.as_bytes(), &mut out);
            assert_eq!(out, expected);
        }
    }

    #[test]
    fn script_json_cannot_close_the_script_element() {
        let text = script_json(&json!({"title": "</script><script>alert(1)</script>&"}));
        assert!(!text.contains('<') && !text.contains('>') && !text.contains('&'));
        let back: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(back["title"], "</script><script>alert(1)</script>&");
    }

    #[test]
    fn octave_centre_labels() {
        let labels: Vec<_> =
            crate::model::octave_bands(48000.0, &crate::model::DEFAULT_OCTAVE_CENTERS)
                .iter()
                .map(|b| centre_label(b.0))
                .collect();
        assert_eq!(labels, ["125", "250", "500", "1k", "2k", "4k", "8k", "16k"]);
        assert_eq!(centre_label(31.5 / std::f64::consts::SQRT_2), "32");
    }
}
