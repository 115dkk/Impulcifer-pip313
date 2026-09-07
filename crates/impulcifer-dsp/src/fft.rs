//! NumPy-compatible transforms. Infallible transforms panic on zero lengths;
//! nonfinite samples propagate through arithmetic, as in NumPy.

use realfft::RealFftPlanner;
use rustfft::FftPlanner;
pub use rustfft::num_complex::Complex64;
use std::cell::RefCell;

thread_local! {
    // Plans are cached by length; work buffers retain only their largest capacity
    // per thread, rather than one allocation per cached transform length.
    static REAL_PLANNER: RefCell<RealFftPlanner<f64>> = RefCell::new(RealFftPlanner::new());
    static REAL_WORK: RefCell<RealWork> = RefCell::new(RealWork::default());
    static COMPLEX_PLANNER: RefCell<FftPlanner<f64>> = RefCell::new(FftPlanner::new());
}

#[derive(Default)]
struct RealWork {
    input: Vec<f64>,
    spectrum: Vec<Complex64>,
    scratch: Vec<Complex64>,
}

/// Unnormalized real transform, including the last bin for odd lengths.
pub fn rfft(x: &[f64]) -> Vec<Complex64> {
    assert!(!x.is_empty(), "FFT length must be positive");
    let plan = REAL_PLANNER.with(|p| p.borrow_mut().plan_fft_forward(x.len()));
    REAL_WORK.with(|work| {
        let mut work = work.borrow_mut();
        let RealWork { input, scratch, .. } = &mut *work;
        input.resize(x.len(), 0.0);
        input.copy_from_slice(x);
        scratch.resize(plan.get_scratch_len(), Complex64::default());
        let mut output = plan.make_output_vec();
        plan.process_with_scratch(input, &mut output, scratch)
            .expect("valid FFT buffers");
        output
    })
}

/// Internal consuming transform avoids copying freshly padded/centered data.
pub(crate) fn rfft_owned(mut x: Vec<f64>) -> Vec<Complex64> {
    assert!(!x.is_empty(), "FFT length must be positive");
    let plan = REAL_PLANNER.with(|p| p.borrow_mut().plan_fft_forward(x.len()));
    let mut output = plan.make_output_vec();
    REAL_WORK.with(|work| {
        let mut work = work.borrow_mut();
        work.scratch
            .resize(plan.get_scratch_len(), Complex64::default());
        plan.process_with_scratch(&mut x, &mut output, &mut work.scratch)
            .expect("valid FFT buffers");
    });
    output
}

/// Normalized real inverse with explicit original length. Like NumPy, crop or
/// zero-pad the spectrum, and ignore imaginary DC (and even-length Nyquist).
/// Panics if `n == 0` or the spectrum is empty. NumPy 2.4.6 empty-spectrum
/// output was allocator-dependent on the oracle machine, so reject it rather
/// than treating uninitialized values as a numerical contract.
pub fn irfft(spec: &[Complex64], n: usize) -> Vec<f64> {
    assert!(n > 0, "FFT length must be positive");
    assert!(!spec.is_empty(), "inverse FFT spectrum must be nonempty");
    let plan = REAL_PLANNER.with(|p| p.borrow_mut().plan_fft_inverse(n));
    let mut output = plan.make_output_vec();
    REAL_WORK.with(|work| {
        let mut work = work.borrow_mut();
        let RealWork {
            spectrum, scratch, ..
        } = &mut *work;
        spectrum.resize(n / 2 + 1, Complex64::default());
        let len = spectrum.len().min(spec.len());
        spectrum[..len].copy_from_slice(&spec[..len]);
        spectrum[len..].fill(Complex64::default());
        spectrum[0].im = 0.0;
        if n.is_multiple_of(2) {
            spectrum[n / 2].im = 0.0;
        }
        scratch.resize(plan.get_scratch_len(), Complex64::default());
        plan.process_with_scratch(spectrum, &mut output, scratch)
            .expect("valid Hermitian spectrum");
    });
    output.iter_mut().for_each(|v| *v /= n as f64);
    output
}

/// Unnormalized complex forward transform. Panics on empty input.
pub fn fft(x: &[Complex64]) -> Vec<Complex64> {
    assert!(!x.is_empty(), "FFT length must be positive");
    let mut result = x.to_vec();
    COMPLEX_PLANNER
        .with(|p| p.borrow_mut().plan_fft_forward(x.len()))
        .process(&mut result);
    result
}

/// Complex inverse normalized by 1/N. Panics on empty input.
pub fn ifft(x: &[Complex64]) -> Vec<Complex64> {
    assert!(!x.is_empty(), "FFT length must be positive");
    let mut result = x.to_vec();
    COMPLEX_PLANNER
        .with(|p| p.borrow_mut().plan_fft_inverse(x.len()))
        .process(&mut result);
    result.iter_mut().for_each(|v| *v /= x.len() as f64);
    result
}

/// Smallest 235-smooth length >= n, matching scipy.fftpack; 0 returns 0.
/// Panics if no such length is representable by usize.
pub fn next_fast_len_legacy(n: usize) -> usize {
    if n <= 1 {
        return n;
    }
    let mut remainder = n;
    for factor in [2, 3, 5] {
        while remainder.is_multiple_of(factor) {
            remainder /= factor;
        }
    }
    if remainder == 1 {
        return n;
    }
    let mut best = None;
    let mut p5 = 1_usize;
    loop {
        let mut p35 = p5;
        loop {
            let mut candidate = p35;
            while candidate < n {
                let Some(next) = candidate.checked_mul(2) else {
                    break;
                };
                candidate = next;
            }
            if candidate >= n {
                best = Some(best.map_or(candidate, |b: usize| b.min(candidate)));
            }
            if p35 >= n {
                break;
            }
            let Some(next) = p35.checked_mul(3) else {
                break;
            };
            p35 = next;
        }
        if p5 >= n {
            break;
        }
        let Some(next) = p5.checked_mul(5) else { break };
        p5 = next;
    }
    best.expect("next fast length overflows usize")
}

/// scipy.fft.next_fast_len(n, real=True), pinned to the 235 factor set.
/// https://docs.scipy.org/doc/scipy-1.16.0/reference/generated/scipy.fft.prev_fast_len.html
/// explicitly lists real 2,3,5 versus complex 2,3,5,7,11. P03's real 235711
/// shortcut is incorrect. Also verified on installed SciPy 1.18.0: 7 -> 8,
/// 11 -> 12, 295270 -> 300000. Backend/version changes can change this rule.
/// Zero and overflow policies are the same as `next_fast_len_legacy`.
pub fn next_fast_len(n: usize) -> usize {
    next_fast_len_legacy(n)
}

/// core.audio_io.magnitude_response: first ceil(N/2) bins, no epsilon;
/// exact zero magnitudes return -infinity. Panics on empty input.
pub fn magnitude_response(x: &[f64]) -> Vec<f64> {
    assert!(!x.is_empty(), "FFT length must be positive");
    let plan = REAL_PLANNER.with(|p| p.borrow_mut().plan_fft_forward(x.len()));
    REAL_WORK.with(|work| {
        let mut work = work.borrow_mut();
        let RealWork {
            input,
            spectrum,
            scratch,
        } = &mut *work;
        input.resize(x.len(), 0.0);
        input.copy_from_slice(x);
        spectrum.resize(x.len() / 2 + 1, Complex64::default());
        scratch.resize(plan.get_scratch_len(), Complex64::default());
        plan.process_with_scratch(input, spectrum, scratch)
            .expect("valid FFT buffers");
        spectrum[..x.len().div_ceil(2)]
            .iter()
            .map(|v| {
                let square = v.norm_sqr();
                // 20 log10(|z|) = 10 log10(|z|^2). Preserve hypot's
                // range handling if squaring underflows or overflows.
                if square.is_normal() {
                    10.0 * square.log10()
                } else {
                    20.0 * v.norm().log10()
                }
            })
            .collect()
    })
}
