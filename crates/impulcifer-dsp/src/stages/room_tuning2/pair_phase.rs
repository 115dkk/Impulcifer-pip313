//! Corrected low-frequency phase comparison between speaker pairs.
use crate::fft::Complex64;

const CENTRES: [f64; 6] = [50.0, 63.0, 80.0, 100.0, 125.0, 160.0];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairPhaseBand {
    pub low_hz: f64,
    pub high_hz: f64,
    pub phase_deg: f64,
    pub loss_db: f64,
}

#[derive(Clone, Debug)]
pub struct PairPhaseResponse {
    pub speaker: String,
    pub left: Vec<Complex64>,
    pub right: Vec<Complex64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PairPhaseReport {
    pub a: String,
    pub b: String,
    pub left: Vec<PairPhaseBand>,
    pub right: Vec<PairPhaseBand>,
    pub worst_db: f64,
}

pub fn pair_phase_edges() -> [f64; 7] {
    [
        40.0,
        (CENTRES[0] * CENTRES[1]).sqrt(),
        (CENTRES[1] * CENTRES[2]).sqrt(),
        (CENTRES[2] * CENTRES[3]).sqrt(),
        (CENTRES[3] * CENTRES[4]).sqrt(),
        (CENTRES[4] * CENTRES[5]).sqrt(),
        200.0,
    ]
}

/// Compute every finite 40–200 Hz phase/loss band for one speaker pair.
pub fn pair_phase_bands(
    frequencies: &[f64],
    a: &[Complex64],
    b: &[Complex64],
) -> Vec<PairPhaseBand> {
    assert_eq!(frequencies.len(), a.len());
    assert_eq!(a.len(), b.len());
    let edges = pair_phase_edges();
    let mut bands = Vec::new();
    for index in 0..6 {
        let mut cross = Complex64::new(0.0, 0.0);
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        let mut count = 0;
        for bin in 0..frequencies.len() {
            let frequency = frequencies[bin];
            let inside = frequency >= edges[index]
                && if index == 5 {
                    frequency <= edges[index + 1]
                } else {
                    frequency < edges[index + 1]
                };
            if !inside {
                continue;
            }
            count += 1;
            cross += a[bin] * b[bin].conj();
            let sum = a[bin] + b[bin];
            numerator += sum.re * sum.re + sum.im * sum.im;
            let magnitudes = a[bin].norm() + b[bin].norm();
            denominator += magnitudes * magnitudes;
        }
        if count == 0 {
            continue;
        }
        let mut phase = cross.im.atan2(cross.re).to_degrees();
        if phase <= -180.0 {
            phase = 180.0;
        }
        if !phase.is_finite() || !numerator.is_finite() || !denominator.is_finite() {
            continue;
        }
        let loss = if denominator == 0.0 {
            0.0
        } else {
            (10.0 * (numerator / denominator).log10()).max(-30.0)
        };
        bands.push(PairPhaseBand {
            low_hz: edges[index],
            high_hz: edges[index + 1],
            phase_deg: phase,
            loss_db: loss,
        });
    }
    bands
}

/// Keep the six worst speaker pairs, with only bands at or below −6 dB.
pub fn pair_phase_report(
    frequencies: &[f64],
    responses: &[PairPhaseResponse],
) -> Vec<PairPhaseReport> {
    let mut report = Vec::new();
    for a in 0..responses.len() {
        for b in a + 1..responses.len() {
            let left: Vec<_> =
                pair_phase_bands(frequencies, &responses[a].left, &responses[b].left)
                    .into_iter()
                    .filter(|band| band.loss_db <= -6.0)
                    .collect();
            let right: Vec<_> =
                pair_phase_bands(frequencies, &responses[a].right, &responses[b].right)
                    .into_iter()
                    .filter(|band| band.loss_db <= -6.0)
                    .collect();
            if left.is_empty() && right.is_empty() {
                continue;
            }
            let worst_db = left
                .iter()
                .chain(&right)
                .map(|band| band.loss_db)
                .min_by(f64::total_cmp)
                .unwrap();
            report.push((
                a,
                b,
                PairPhaseReport {
                    a: responses[a].speaker.clone(),
                    b: responses[b].speaker.clone(),
                    left,
                    right,
                    worst_db,
                },
            ));
        }
    }
    report.sort_by(|left, right| {
        left.2
            .worst_db
            .total_cmp(&right.2.worst_db)
            .then(left.0.cmp(&right.0))
            .then(left.1.cmp(&right.1))
    });
    report.truncate(6);
    report.into_iter().map(|(_, _, report)| report).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(name: &str, phase_deg: f64) -> PairPhaseResponse {
        let value = Complex64::from_polar(1.0, phase_deg.to_radians());
        PairPhaseResponse {
            speaker: name.into(),
            left: vec![value; 6],
            right: vec![value; 6],
        }
    }

    #[test]
    fn inverted_bass_is_reported_and_identical_bass_is_not() {
        let frequencies = [50.0, 63.0, 80.0, 100.0, 125.0, 160.0];
        assert!(
            pair_phase_report(&frequencies, &[response("A", 0.0), response("B", 0.0)]).is_empty()
        );
        let report = pair_phase_report(&frequencies, &[response("A", 0.0), response("B", 180.0)]);
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].left.len(), 6);
        assert!(report[0].left.iter().all(|band| band.loss_db == -30.0));
    }

    #[test]
    fn report_orders_by_loss_then_speaker_order_and_keeps_six() {
        let frequencies = [50.0, 63.0, 80.0, 100.0, 125.0, 160.0];
        let responses: Vec<_> = [0.0, 30.0, 60.0, 180.0, 210.0, 240.0]
            .into_iter()
            .enumerate()
            .map(|(index, phase)| response(&format!("R{index}"), phase))
            .collect();
        let report = pair_phase_report(&frequencies, &responses);
        assert_eq!(report.len(), 6);
        assert_eq!((&report[0].a, &report[0].b), (&"R0".into(), &"R3".into()));
        assert!(
            report
                .windows(2)
                .all(|pair| pair[0].worst_db <= pair[1].worst_db)
        );
    }
}
