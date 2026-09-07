#![forbid(unsafe_code)]

#[path = "../benches/perf.rs"]
mod perf;

#[test]
fn bench_smoke_impulcifer_io() {
    let dir = perf::TempDir::new();
    perf::run(&dir.0, perf::Mode::Smoke);
}

#[test]
fn quantization_matches_round_ties_even_at_boundaries() {
    let mut values = vec![-f64::MAX, f64::MAX, -0.0, 0.0, f64::MIN_POSITIVE];
    let mut state = 7u64;
    for _ in 0..100000 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let value = f64::from_bits(state);
        if value.is_finite() {
            values.push(value);
        }
        let half = ((state as i32) as f64 + 0.5) / 2147483648.0;
        values.extend([half.next_down(), half, half.next_up()]);
    }
    let expected: Vec<_> = values
        .iter()
        .map(|x| ((x * 2147483648.0).round_ties_even() as i32) as f64 / 2147483648.0)
        .collect();
    assert_eq!(impulcifer_io::pcm32_round_trip(&[values])[0], expected);
}

#[test]
fn block_io_matches_scalar_conversion_all_depths_and_layouts() {
    let dir = perf::TempDir::new();
    let path = dir.0.join("blocks.wav");
    for channels in [1, 2, 3, 8, 30, 32] {
        // Cross both read/write block boundaries with a partial last tile.
        let mut tracks = perf::input(channels, 262144 / (channels * 2) + 257);
        for track in &mut tracks {
            track[0] = -1.5;
            track[1] = 1.5;
            track[2] = 0.5 / 2147483648.0;
        }
        for bits in [16, 24, 32] {
            impulcifer_io::write_wav(&path, 48000, &tracks, bits).unwrap();
            let result = impulcifer_io::read_wav(&path).unwrap();
            for (actual, input) in result.tracks.iter().zip(&tracks) {
                for (&actual, &input) in actual.iter().zip(input) {
                    let quantized = (input * 2147483648.0).round_ties_even() as i32;
                    let expected = (quantized >> (32 - bits)) as f64 / (1u64 << (bits - 1)) as f64;
                    assert_eq!(actual, expected);
                }
            }
            let bytes = std::fs::read(&path).unwrap();
            std::fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
            assert!(impulcifer_io::read_wav(&path).is_err());
        }
    }
}

#[test]
fn parallel_pcm_decode_matches_scalar_at_dispatch_boundary() {
    let dir = perf::TempDir::new();
    let path = dir.0.join("parallel-pcm.wav");
    for channels in [30, 32] {
        for frames in [32767, 32768, 32769] {
            let tracks = perf::input(channels, frames);
            for bits in [16, 24, 32] {
                impulcifer_io::write_wav(&path, 48000, &tracks, bits).unwrap();
                let decoded = impulcifer_io::read_wav(&path).unwrap();
                assert_eq!(decoded.tracks.len(), channels);
                for (actual, input) in decoded.tracks.iter().zip(&tracks) {
                    assert_eq!(actual.len(), frames);
                    for (&actual, &input) in actual.iter().zip(input) {
                        let quantized = (input * 2147483648.0).round_ties_even() as i32;
                        let expected =
                            (quantized >> (32 - bits)) as f64 / (1u64 << (bits - 1)) as f64;
                        assert_eq!(actual, expected);
                    }
                }
                let bytes = std::fs::read(&path).unwrap();
                std::fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
                assert!(impulcifer_io::read_wav(&path).is_err());
            }
        }
    }
}

#[test]
fn parallel_float_decode_preserves_samples_and_ignores_trailing_chunks() {
    let dir = perf::TempDir::new();
    let path = dir.0.join("parallel-float.wav");
    let patterns = [0.0_f64, -0.0, 0.5, -0.5, f64::INFINITY, f64::NEG_INFINITY];
    for channels in [30, 32] {
        for bits in [32, 64] {
            let frames = 32769;
            perf::float_fixture_sized(&path, channels, frames);
            let mut bytes = std::fs::read(&path).unwrap();
            if bits == 64 {
                bytes.truncate(44);
                bytes[28..32].copy_from_slice(&(48000 * channels as u32 * 8).to_le_bytes());
                bytes[32..34].copy_from_slice(&(channels as u16 * 8).to_le_bytes());
                bytes[34..36].copy_from_slice(&64u16.to_le_bytes());
                bytes[40..44].copy_from_slice(&((channels * frames * 8) as u32).to_le_bytes());
                for i in 0..channels * frames {
                    bytes.extend(patterns[i % patterns.len()].to_le_bytes());
                }
            } else {
                let patterns = [
                    0u32, 0x80000000, 0x3f000000, 0xbf000000, 0x7f800000, 0xff800000,
                ];
                for (i, sample) in bytes[44..].chunks_exact_mut(4).enumerate() {
                    sample.copy_from_slice(&patterns[i % patterns.len()].to_le_bytes());
                }
            }
            bytes.extend(b"JUNK");
            bytes.extend(4u32.to_le_bytes());
            bytes.extend([255; 4]);
            let riff_size = (bytes.len() - 8) as u32;
            bytes[4..8].copy_from_slice(&riff_size.to_le_bytes());
            std::fs::write(&path, bytes).unwrap();
            let decoded = impulcifer_io::read_wav(&path).unwrap();
            assert_eq!(decoded.tracks.len(), channels);
            for (channel, track) in decoded.tracks.iter().enumerate() {
                assert_eq!(track.len(), frames);
                for (frame, sample) in track.iter().enumerate() {
                    let expected = patterns[(frame * channels + channel) % patterns.len()];
                    assert_eq!(sample.to_bits(), expected.to_bits());
                }
            }
        }
    }
}

#[test]
fn float_blocks_preserve_ieee_widening_and_special_values() {
    let dir = perf::TempDir::new();
    let path = dir.0.join("float.wav");
    let patterns = [
        0u32, 0x80000000, 1, 0x007fffff, 0x00800000, 0x3f000000, 0xbf000000, 0x7f7fffff,
        0x7f800000, 0xff800000, 0x7fc00001,
    ];
    for channels in [1, 2, 3, 8, 30, 32] {
        let frames = 262144 / (channels * 4) + 257;
        perf::float_fixture_sized(&path, channels, frames);
        let mut bytes = std::fs::read(&path).unwrap();
        for (i, sample) in bytes[44..].chunks_exact_mut(4).enumerate() {
            sample.copy_from_slice(&patterns[i % patterns.len()].to_le_bytes());
        }
        std::fs::write(&path, bytes).unwrap();
        let decoded = impulcifer_io::read_wav(&path).unwrap();
        for (channel, track) in decoded.tracks.iter().enumerate() {
            for (frame, &actual) in track.iter().enumerate() {
                let bits = patterns[(frame * channels + channel) % patterns.len()];
                // Integer IEEE decomposition supplies an independent f64 oracle.
                let sign = if bits >> 31 == 0 { 1.0 } else { -1.0 };
                let exponent = (bits >> 23) & 255;
                let mantissa = bits & 0x7fffff;
                if exponent == 255 && mantissa != 0 {
                    assert!(actual.is_nan());
                    continue;
                }
                let expected = if exponent == 255 {
                    sign * f64::INFINITY
                } else if exponent == 0 {
                    sign * mantissa as f64 * 2.0_f64.powi(-149)
                } else {
                    sign * (mantissa + (1 << 23)) as f64 * 2.0_f64.powi(exponent as i32 - 150)
                };
                assert_eq!(actual.to_bits(), expected.to_bits());
            }
        }
    }
}
