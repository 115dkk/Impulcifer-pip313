use super::sweep::SweepSpec;
use impulcifer_types::constants::{hdmi_track_order, sweep_track_order};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelOrder {
    #[default]
    Auto,
    Standard,
    Hdmi,
}

impl ChannelOrder {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "standard" => Some(Self::Standard),
            "hdmi" => Some(Self::Hdmi),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Standard => "standard",
            Self::Hdmi => "hdmi",
        }
    }
}

/// Names of the macOS output devices on HDMI or DisplayPort, read from CoreAudio
/// (`impulcifer_audio_io::transport`). Empty on other platforms and in builds
/// without the native audio backend.
pub fn hdmi_outputs() -> Vec<String> {
    impulcifer_audio_io::transport::hdmi_output_names()
}

/// Resolves the named layout when playback has a defined macOS HDMI order.
pub fn playback_layout(
    sweep: Option<&SweepSpec>,
    play_path: Option<&str>,
    channels: usize,
) -> Option<String> {
    let layout = if let Some(spec) = sweep {
        spec.tracks.clone()
    } else {
        let info = impulcifer_io::sweep_files::parse_sweep_file_name(play_path?)?;
        info.layout
    };
    let order = hdmi_track_order(&layout)?;
    (order.len() == channels).then_some(layout)
}

/// Moves each standard-order playback row to the same slot in the HDMI order.
pub fn to_hdmi_order(tracks: Vec<Vec<f64>>, layout: &str) -> Vec<Vec<f64>> {
    let Some(standard) = sweep_track_order(layout) else {
        return tracks;
    };
    let Some(hdmi) = hdmi_track_order(layout) else {
        return tracks;
    };
    if tracks.len() != standard.len() {
        return tracks;
    }
    hdmi.into_iter()
        .map(|slot| {
            let index = standard
                .iter()
                .position(|candidate| *candidate == slot)
                .expect("HDMI order is a permutation of standard order");
            tracks[index].clone()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_hdmi_order_moves_each_row_to_its_hdmi_slot() {
        let rows = |count: usize| (0..count).map(|index| vec![index as f64]).collect();
        assert_eq!(
            to_hdmi_order(rows(8), "7.1"),
            vec![
                vec![0.0],
                vec![1.0],
                vec![3.0],
                vec![2.0],
                vec![6.0],
                vec![7.0],
                vec![4.0],
                vec![5.0]
            ]
        );
        assert_eq!(
            to_hdmi_order(rows(6), "5.1"),
            vec![
                vec![0.0],
                vec![1.0],
                vec![3.0],
                vec![2.0],
                vec![4.0],
                vec![5.0]
            ]
        );
        assert_eq!(to_hdmi_order(rows(7), "7.1"), rows(7));
        assert_eq!(to_hdmi_order(rows(2), "stereo"), rows(2));
    }

    #[test]
    fn playback_layout_from_spec_and_file_name() {
        let spec = SweepSpec {
            tracks: "7.1".into(),
            ..SweepSpec::default()
        };
        assert_eq!(
            playback_layout(Some(&spec), None, 8).as_deref(),
            Some("7.1")
        );
        assert_eq!(playback_layout(Some(&spec), None, 7), None);
        assert_eq!(
            playback_layout(
                None,
                Some("/tmp/sweep-seg-FL,FR-5.1-1.00s-48000Hz-32bit-2.00Hz-24000Hz.wav"),
                6
            )
            .as_deref(),
            Some("5.1")
        );
        assert_eq!(
            playback_layout(
                None,
                Some("sweep-seg-FL,FR-5.1-1.00s-48000Hz-32bit-2.00Hz-24000Hz.wav"),
                8
            ),
            None
        );
        assert_eq!(playback_layout(None, Some("other.wav"), 8), None);
    }
}
