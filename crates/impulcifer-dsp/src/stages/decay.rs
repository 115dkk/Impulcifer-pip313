//! Per-speaker decay stage.
use crate::{DspError, hrir::Hrir};
/// Python _stage_decay/process_decay_worker, core/pipeline.py:702-724,
/// core/parallel_workers.py:24-39; p10_decay.
pub fn adjust_decay(hrir: &mut Hrir, targets: &[(String, f64)]) -> Result<(), DspError> {
    for s in &mut hrir.speakers {
        if let Some((_, target)) = targets.iter().find(|(name, _)| name == &s.speaker) {
            if !target.is_finite() || *target <= 0.0 {
                return Err(DspError::InvalidArgument(
                    "decay target must be positive".into(),
                ));
            }
            for ir in s.left.iter_mut().chain(s.right.iter_mut()) {
                ir.adjust_decay(*target);
            }
        }
    }
    Ok(())
}
