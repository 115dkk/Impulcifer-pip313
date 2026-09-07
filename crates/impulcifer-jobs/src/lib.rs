#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Job registry: one active job, per-job seq journal bounded to
//! `MAX_JOB_EVENTS`, `poll(job_id, after_seq)` replay and cooperative
//! cancellation. Port of the job half of `application/impulcifer_service.py`.

pub mod registry;
