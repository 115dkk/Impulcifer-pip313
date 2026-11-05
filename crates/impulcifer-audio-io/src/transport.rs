//! Which output devices are HDMI or DisplayPort (ADR 0012).
//!
//! A Mac exposes the HDMI slots of such an output directly as its device
//! channels (CEA-861: LFE on 3, FC on 4), so the recorder plays a 5.1 or 7.1
//! sweep in that order there. The transport type comes from CoreAudio through
//! coreaudio-rs, the same crate and version cpal uses on macOS; its helpers are
//! safe functions, and the names are the ones cpal reports (both read
//! `kAudioDevicePropertyDeviceNameCFString` through `get_device_name`).

/// `kAudioDeviceTransportTypeHDMI` ('hdmi').
pub const TRANSPORT_HDMI: u32 = u32::from_be_bytes(*b"hdmi");
/// `kAudioDeviceTransportTypeDisplayPort` ('dprt').
pub const TRANSPORT_DISPLAY_PORT: u32 = u32::from_be_bytes(*b"dprt");

/// Whether a CoreAudio transport type is HDMI or DisplayPort.
pub fn is_hdmi_transport(transport: u32) -> bool {
    transport == TRANSPORT_HDMI || transport == TRANSPORT_DISPLAY_PORT
}

/// Names of the audio devices on HDMI or DisplayPort. Empty on Windows and
/// Linux, without the native backend, and when CoreAudio cannot be read.
pub fn hdmi_output_names() -> Vec<String> {
    #[cfg(all(feature = "native", target_os = "macos"))]
    {
        use coreaudio::audio_unit::macos_helpers::{
            get_audio_device_ids, get_device_name, get_device_transport_type,
        };
        get_audio_device_ids()
            .unwrap_or_default()
            .into_iter()
            .filter(|&id| get_device_transport_type(id).is_ok_and(is_hdmi_transport))
            .filter_map(|id| get_device_name(id).ok())
            .collect()
    }
    #[cfg(not(all(feature = "native", target_os = "macos")))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hdmi_and_display_port_transports_are_recognised() {
        assert_eq!(TRANSPORT_HDMI, 0x6864_6d69);
        assert_eq!(TRANSPORT_DISPLAY_PORT, 0x6470_7274);
        assert!(is_hdmi_transport(TRANSPORT_HDMI));
        assert!(is_hdmi_transport(TRANSPORT_DISPLAY_PORT));
        for other in [*b"bltn", *b"usb ", *b"grup", *b"virt", *b"blue", *b"airp"] {
            assert!(!is_hdmi_transport(u32::from_be_bytes(other)));
        }
    }

    /// Reads CoreAudio on the macOS CI runner (which has no HDMI output): the
    /// enumeration runs and every name it returns is a real device name.
    #[cfg(all(feature = "native", target_os = "macos"))]
    #[test]
    fn hdmi_output_names_reads_coreaudio_on_this_mac() {
        let names = hdmi_output_names();
        assert!(
            names.iter().all(|name| !name.trim().is_empty()),
            "{names:?}"
        );
        let all = coreaudio::audio_unit::macos_helpers::get_audio_device_ids().unwrap();
        assert!(names.len() <= all.len());
    }

    #[cfg(not(all(feature = "native", target_os = "macos")))]
    #[test]
    fn hdmi_output_names_is_empty_off_macos() {
        assert!(hdmi_output_names().is_empty());
    }
}
