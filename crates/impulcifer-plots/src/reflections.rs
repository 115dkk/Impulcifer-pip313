//! Plot-only early-reflection envelope; never modifies the pipeline's IR.
use std::collections::VecDeque;

#[derive(Debug)]
pub(crate) struct Reflections {
    pub time: Vec<f64>,
    pub db: Vec<f64>,
    pub echoes: Vec<(f64, f64)>,
    /// Earliest qualifying echo, even when stronger later echoes take all labels.
    pub first_echo: Option<(f64, f64)>,
    pub floor: f64,
}

/// A 0.2 ms peak-hold envelope bridges carrier zero crossings. Candidates must
/// fall after the direct lobe, have >6 dB prominence on BOTH sides within 1 ms,
/// and exceed -30 dB relative to direct sound. Keep at most three, 0.5 ms apart.
/// These are display heuristics, not a room-acoustics measurement or DSP stage.
pub(crate) fn reflections(ir: &[f64], fs: u32, floor: Option<f64>) -> Option<Reflections> {
    if ir.is_empty() || fs == 0 {
        return None;
    }
    let magnitude: Vec<_> = ir.iter().map(|v| v.abs()).collect();
    let maximum = magnitude.iter().copied().fold(0., f64::max);
    if maximum == 0. {
        return None;
    }
    // Use the first substantial local peak, not a potentially stronger later echo.
    let onset = (0..magnitude.len()).find(|&i| {
        magnitude[i] >= maximum * 0.12589
            && (i == 0 || magnitude[i] > magnitude[i - 1])
            && (i + 1 == magnitude.len() || magnitude[i] >= magnitude[i + 1])
    })?;
    // Stay on this peak's plateau rather than searching a fixed time window,
    // which could absorb a distinct, stronger early reflection into direct sound.
    let mut plateau_end = onset;
    while plateau_end + 1 < magnitude.len() && magnitude[plateau_end + 1] == magnitude[onset] {
        plateau_end += 1;
    }
    let direct = (onset + plateau_end) / 2;
    let reference = magnitude[direct];
    let radius = (f64::from(fs) * 0.0001).round().max(1.) as usize;
    let end = (direct + (f64::from(fs) * 0.025).floor() as usize + 1).min(ir.len());
    let mut queue = VecDeque::<usize>::new();
    let mut next = direct.saturating_sub(radius);
    let mut db = Vec::with_capacity(end - direct);
    for i in direct..end {
        while next < (i + radius + 1).min(ir.len()) {
            while queue
                .back()
                .is_some_and(|&j| magnitude[j] <= magnitude[next])
            {
                queue.pop_back();
            }
            queue.push_back(next);
            next += 1;
        }
        while queue.front().is_some_and(|&j| j < i.saturating_sub(radius)) {
            queue.pop_front();
        }
        let peak = magnitude[*queue.front()?];
        db.push(20. * (peak / reference).max(1e-6).log10());
    }
    let time: Vec<_> = (direct..end)
        .map(|i| (i - direct) as f64 * 1000. / f64::from(fs))
        .collect();
    // The held direct peak and its falling envelope belong to the direct lobe.
    // A new rise after that fall is eligible at any later time, including <1 ms;
    // the two-sided prominence test still requires a genuinely separating valley.
    let direct_lobe_end = db.windows(2).position(|w| w[1] > w[0]).unwrap_or(db.len());
    let neighborhood = (f64::from(fs) * 0.001).round().max(1.) as usize;
    let mut peaks = Vec::new();
    let mut i = 1;
    while i + 1 < db.len() {
        if db[i] > db[i - 1] {
            let mut right = i;
            while right + 1 < db.len() && db[right + 1] == db[i] {
                right += 1;
            }
            if right + 1 < db.len() && db[right + 1] < db[i] {
                let center = (i + right) / 2;
                let before = db[i.saturating_sub(neighborhood)..i]
                    .iter()
                    .copied()
                    .fold(f64::INFINITY, f64::min);
                let after = db[right + 1..(right + neighborhood + 1).min(db.len())]
                    .iter()
                    .copied()
                    .fold(f64::INFINITY, f64::min);
                if i > direct_lobe_end && db[center] > -30. && db[center] - before.max(after) > 6. {
                    peaks.push((time[center], db[center]));
                }
            }
            i = right;
        }
        i += 1;
    }
    let first_echo = peaks.first().copied();
    peaks.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.total_cmp(&b.0)));
    let mut echoes: Vec<(f64, f64)> = Vec::new();
    for peak in peaks {
        if echoes.iter().all(|e| (e.0 - peak.0).abs() >= 0.5) {
            echoes.push(peak);
        }
        if echoes.len() == 3 {
            break;
        }
    }
    echoes.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Supplied Lundeby floor is relative to the GLOBAL maximum; convert its
    // reference to direct sound. Without it, estimate RMS from the last 10%.
    let floor = floor.map_or_else(
        || {
            let tail = &magnitude[magnitude.len() * 9 / 10..];
            let power =
                tail.iter().map(|v| (v / reference).powi(2)).sum::<f64>() / tail.len() as f64;
            10. * power.max(1e-12).log10()
        },
        |v| v + 20. * (maximum / reference).log10(),
    );
    Some(Reflections {
        time,
        db,
        echoes,
        first_echo,
        floor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<f64> {
        let mut ir = vec![0.001; 4800];
        ir[48] = 1.;
        for (ms, db) in [
            (3.2, -12.),
            (7., -8.),
            (12., -20.),
            (18., -24.),
            (24., -31.),
            (28., -3.),
        ] {
            ir[48 + (ms * 48.) as usize] = 10_f64.powf(db / 20.);
        }
        ir
    }
    #[test]
    fn strongest_three_echoes_are_direct_relative_and_time_ordered() {
        let r = reflections(&fixture(), 48000, Some(-60.)).unwrap();
        assert_eq!(r.db[0], 0.);
        assert_eq!(r.time[0], 0.);
        assert_eq!(r.time.last(), Some(&25.));
        assert_eq!(r.echoes.len(), 3);
        for (actual, expected) in r
            .echoes
            .iter()
            .zip([(3.1875, -12.), (7., -8.), (12., -20.)])
        {
            assert!((actual.0 - expected.0).abs() < 0.03);
            assert!((actual.1 - expected.1).abs() < 1e-9);
        }
        assert_eq!(r.floor, -60.);
    }
    #[test]
    fn carrier_oscillations_and_shallow_bumps_are_not_echoes() {
        let mut ir: Vec<_> = (0..4800)
            .map(|i| (-i as f64 / 800.).exp() * (i as f64 * 0.8).cos())
            .collect();
        assert!(reflections(&ir, 48000, None).unwrap().echoes.is_empty());
        ir.fill(0.1);
        ir[0] = 1.;
        ir[480] = 0.15; // 3.5 dB above surroundings, not a prominent echo.
        assert!(reflections(&ir, 48000, None).unwrap().echoes.is_empty());
    }
    #[test]
    fn silence_short_inputs_and_stronger_later_echo_are_handled() {
        assert!(reflections(&[], 48000, None).is_none());
        assert!(reflections(&[0.; 8], 48000, None).is_none());
        assert!(reflections(&[1.], 48000, None).unwrap().echoes.is_empty());
        let mut ir = fixture();
        ir[48 + 480] = 2.;
        let r = reflections(&ir, 48000, Some(-60.)).unwrap();
        assert_eq!(r.time[0], 0.);
        assert!(r.echoes.iter().any(|&(t, db)| t == 10. && db > 6.));
        assert!((r.floor - (-60. + 20. * 2_f64.log10())).abs() < 1e-9);
    }
    #[test]
    fn distinct_submillisecond_echoes_follow_the_actual_direct_lobe() {
        for offset in [14, 34] {
            // 0.29 ms and 0.71 ms at 48 kHz.
            let mut ir = vec![0.001; 4800];
            ir[48] = 1.;
            ir[48 + offset] = 0.25;
            let r = reflections(&ir, 48000, Some(-60.)).unwrap();
            let (time, db) = r.first_echo.unwrap();
            assert!((time - offset as f64 / 48.).abs() < 0.03);
            assert!((db - 20. * 0.25_f64.log10()).abs() < 1e-9);
            assert_eq!(r.echoes, vec![(time, db)]);
        }
        // A separate stronger peak must not replace the direct reference even
        // when it lies inside the former 0.5 ms direct-search window.
        let mut ir = vec![0.001; 4800];
        ir[48] = 1.;
        ir[62] = 2.;
        let r = reflections(&ir, 48000, None).unwrap();
        assert_eq!(r.db[0], 0.);
        assert!(r.first_echo.is_some_and(|(t, db)| t < 0.5 && db > 6.));
    }
    #[test]
    fn earliest_qualified_echo_is_kept_when_three_later_echoes_are_stronger() {
        let mut ir = fixture();
        ir[82] = 10_f64.powf(-25. / 20.);
        let r = reflections(&ir, 48000, None).unwrap();
        let (time, db) = r.first_echo.unwrap();
        assert!((time - 34. / 48.).abs() < 0.03);
        assert!((db + 25.).abs() < 1e-9);
        assert_eq!(r.echoes.len(), 3);
        assert!(r.echoes.iter().all(|&(t, level)| t > time && level > db));
    }
    #[test]
    fn threshold_and_prominence_are_strict() {
        let mut ir = vec![0.001; 4800];
        ir[0] = 1.;
        ir[480] = 10_f64.powf(-30. / 20.);
        assert!(reflections(&ir, 48000, None).unwrap().echoes.is_empty());
        ir.fill(0.1);
        ir[0] = 1.;
        ir[480] = 0.1 * 10_f64.powf(5.99 / 20.);
        assert!(reflections(&ir, 48000, None).unwrap().echoes.is_empty());
    }
}
