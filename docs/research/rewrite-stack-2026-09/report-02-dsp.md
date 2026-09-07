# Impulcifer rewrite: DSP and numerical compatibility

Research date: **2026-09-07**. Dimension: R2/R9, with limited R3 implications. Source revision **0144dcc1f8a4634607b980d15c64549d4aa6c145**, project **2.13.3**.

## 1 Verdict

1. **[I] DSP-only ranking: C++ first, Rust second, C#/.NET third, Go fourth, Kotlin/JVM fifth, Dart sixth.** This is not a GUI-shell ranking.
2. **[I] Prefer C++ + Eigen + pocketfft + an explicit SciPy-compatibility layer.** Ceres is a fit-quality replacement, not a TRF clone; GPL FFT dependencies are unnecessary.
3. **[I] Rust/Tauri is viable**, but sci-rs is not a drop-in SciPy. Own the spline, resampling, endpoint and bounded-optimizer contracts.
4. **[I] C# is more credible than Go for this workload.** MathNet has actual bounded LM; NWaves has double components and a firwin2-like designer, alongside float-centric APIs.
5. **[I] Go/Wails does not simplify R2.** Gonum provides good FFT/matrix foundations and not-a-knot cubic interpolation, but much signal compatibility remains custom.
6. **[V-local] The brief omits active FITPACK k=2 interpolation and overstates FLOAT32 output:** the inspected common WAV writer selects integer PCM_32. [1–3]
7. **[V] nnresample adds a 32,001-tap null-on-Nyquist design, including a 524,288-point FFT search, not merely standard resample_poly.** [9–12]
8. **[I] Replace cross-language SHA-256 comparison with per-stage goldens plus a versioned waveform/spectral/timing gate, not an arbitrary loose allclose.**
9. **[I] Plan 2–4k C++, 1.8–3.8k Rust, 2–4.2k C#, or 2.8–5.2k Go production compatibility lines**, excluding pipeline translation, tests, GUI/plots and a complete TRF port.
10. **[U] No candidate was compiled or benchmarked here; the tolerances and cost estimates below are proposed acceptance criteria, not measured guarantees.**

## 2 Findings (with sources)

### 2.1 Evidence conventions and scope

**[V]** = verified in linked primary documentation/source; **[V-local]** = verified in local source, cited through the immutable repository revision; **[I]** = inference/recommendation/estimate; **[U]** = unverified. Tags at the start of a paragraph or table cell apply to its claims. An API's existence is not evidence of numerical equivalence. Observed package versions are dated **2026-09-07**; not every observed version is certified latest. Moving master/latest URLs require commit/version pinning before implementation.

**[V-local] Reading coverage:** all 13 requested files were read completely, across the parent and a read-only inventory agent: `autoeq/frequency_response.py` (1,621 lines), `autoeq/biquad.py` (179), `core/impulse_response.py` (189), `hrir.py` (1,092), `room_correction.py` (462), `virtual_bass.py` (199), `microphone_deviation_correction.py` (508), `decay.py` (404), `sweep_detection.py` (245), `sweep_signal.py` (206), `impulse_response_estimator.py` (327), `audio_io.py` (119), `eqapo.py` (941). Additional targeted reads covered plotting, constants, parity tests, dependency declarations and ADR 0001. No repository file was changed. Shell probes requiring approval did not execute; conclusions are source-derived, not runtime measurements. [1–3]

### 2.2 Exact inventory and important corrections

Names below are relative to the repository root. N denotes IR/input length, K FIR taps, M frequency samples, B fitted biquads, fs sample rate. Sizes are formulas or source-derived examples at 48 kHz, not profiling observations.

| Source / lines | Verified operation and numerical contract | Representative dimensions / edge cases |
|---|---|---|
| `audio_io.py:67–113`; `audio_truehd.py:153–185` | [V-local] `soundfile.read` default float64, tracks-by-samples orientation. Magnitude is NumPy rFFT, then first `ceil(N/2)` bins, then `20log10(abs(...))`, without epsilon, scaling or doubling. | Even N excludes Nyquist; odd N retains its final complex bin. N=48,000 gives 24,000 exposed bins; N=48,001 gives 24,001. Spectral zero gives -infinity. |
| `audio_io.py:82–97` | [V-local] Writer maps bit depth to PCM_16/24/32, not IEEE FLOAT. `sweep_signal.py:138–152` intentionally roundtrips PCM_32 in memory, then reads float64. | R3's desired float32 file must be a deliberate output-format change, not assumed current parity. Compare decoded samples separately from headers. |
| `impulse_response_estimator.py:73–151` | [V-local] Exponential sweep, symmetric half-Hann fade, inverse sweep amplitude compensation, full convolution, exact-length complex FFT for inverse normalization; estimation uses `signal.convolve(recording,inverse,mode='same')`. | Default sweep N=295,270, not 295,200 or a power of two. Inverse self-convolution length 590,539, normalization bin `round(length/4)`=147,635. Ingestion segment R=391,270; full conv R+N-1=686,539; same crop starts 147,634. |
| `sweep_detection.py:53–108`; `impulse_response_estimator.py:86–147` | [V-local] Grid P=ceil(log2((fs/2)/5)), U=2 ln(2^P) 2^P; synthesis uses ceil(requested_samples/U), detection nearest grid. Phase uses both unrounded length and rounded sample count. | At 48k P=13, unit approximately 147,634.805 samples; M=1 produces 147,635, M=2 produces 295,270. Default fade-in 11,356 samples. Preserve rounding rules, not just sweep duration labels. |
| `sweep_signal.py:155–193`; estimator `:153–232` | [V-local] Float64 multitrack zero buffers; sequential speaker placement; PCM_32 quantization. | L=96,000+S(295,270+96,000). Mono (1,487270); stereo (2,878540); seven speakers in eight tracks (8,2834890), 181,432,960 bytes before temporaries. |
| `sweep_detection.py:128–178` | [V-local] Entire-file float64 read, max-absolute across tracks, **NumPy direct convolve** with 2,400-point boxcar, threshold 1%, runs/gap merging/median. | NumPy same returns max(input lengths), unlike SciPy first-input same. Gaps under 14,400 samples merge; runs under 24,000 rejected. |
| `impulse_response.py:32–70`; `decay.py:12–41` | [V-local] Normalize search slice; `find_peaks` on positive and negative data with inclusive height 0.12589; return earliest candidate, else absolute argmax. | Empty returns start; max below 1e-20 is silence; endpoints excluded except fallback; plateau midpoint matters. Integer in-place division can fail. |
| `impulse_response.py:110–135`; `hrir.py:858–919` | [V-local] Full FIR convolution N+K-1; `nnresample.resample(data,new_fs,old_fs)`; general left/right FIR application. | Do not silently keep only N samples. Per-ear resampling is parallelizable. Output count ceil(N*new_fs/old_fs). |
| `hrir.py:43–73,457–653` | [V-local] Center uses log-axis **k=1**, including its retry. Normalize summed ear IR spectra, peak or mean dB over 80–6000 Hz; pair-preserving crop, legacy fftpack fast length, Hann fades. | Different ear lengths, exact threshold masks and reductions affect results. Tail fade default 946 samples. Normalization is not simply waveform peak normalization. |
| `hrir.py:655–799` | [V-local] Channel balance generates AutoEQ minimum-phase FIRs or gain-only **4,800-tap unit impulses**; averages IRs in time before FR computation. | Trailing zero taps still increase full-convolution length. Changing to one-tap gain changes shapes and downstream analysis. |
| `hrir.py:921–1001` | [V-local] Full signed cross-correlation of first 30 ms, hand-built lag array, argmax; zero-pad shifts, not circular roll. | 1,440 versus 1,440 samples gives 2,879 correlation values. Unequal-input lag assumption is not a generic correlation_lags contract. |
| `room_correction.py:185–306` | [V-local] Calibration/centering, arithmetic means in **dB**, conservative same-sign masks, fractional-octave smoothing. Correction limit uses the **whole symmetric Hann** between limit/2 and limit. | Approximately 783 grid points for 10 Hz to Nyquist at ratio 1.01. Do not replace that whole Hann with a more sensible monotonic half-window during compatibility porting. |
| `virtual_bass.py:46–175` | [V-local] Butterworth HP4 at 15 Hz; LP4 at crossover repeated to LP8; HP4 repeated to HP8 on original ears; 3 RBJ shelves via tf2sos; zero-state sosfilt; nearest-bin rFFT gain matching. | SOS arrays (2,6), (4,6), (3,6); N is current longest IR. f64 coefficients/output. rFFT includes Nyquist here. For S pairs, 2S+1 crossover spectra; HP originals filtered twice. Delay truncates tails to N. |
| `microphone_deviation_correction.py:106–276` | [V-local] Explicit f64; short peak window, half-Hanns; scipy.fft next_fast_len(max(segment,8192)); rFFT; **linear-frequency magnitude interpolation then square**; mean power and dB ratio; AutoEQ FIR. | Typically 264-window samples, FFT8192/4097 bins, 5.859375-Hz spacing; approximately 713 log-grid values. FIR truncated without fade/renormalization to 2,048 taps. |
| same file `:329–454` | [V-local] Compatibility pair API uses same convolution; HRIR API uses full. | K=2048 same crop advances by 1,023 samples relative to causal full output. Full adds 2,047 samples. Do not unify these accidentally. |
| `decay.py:44–403`; `audio_io.py:116–118` | [V-local] Lundeby power-bin regression, noise estimation, up to five refinements, cumsum running means and Schroeder-style sums, linregress; dB-shaped half-Hann decay window. | Analyze up to 96,000 samples; initially 66 bins of 1,454 at 2 seconds, remainder dropped. Time uses endpoint-inclusive linspace, not exactly arange/fs. EDT/RT20/RT30 are interval durations, not all RT60 extrapolations. Knee and regression-range decisions need exact discrete tests. |
| `eqapo.py:218–473,827–904` | [V-local] RBJ scalar f64 coefficients; arbitrary IIR/FIR **freqz at explicit arbitrary frequencies**, GraphicEQ log-axis np.interp with clamped endpoints. | K=48,000, M≈783 entails about 37.6 million coefficient-frequency terms per channel, not one FFT. Separate raw polynomial response evaluator needed. |
| AutoEQ `:350–516` | [V-local] Bounded dense nonlinear least squares, detailed below; peaking/shelf evaluation in dB; 1,000 SG smoothing iterations for initialization. | Default 20–20,000 Hz ratio1.01 grid has 695 points. Jacobian M×3B or M×B, not necessarily sparse. |
| AutoEQ `:637–705` | [V-local] Legacy fast length, log interpolation, gain doubling, firwin2, homomorphic minimum_phase with **n_fft=len(linear_ir)**. | At fs48k/f_res10: n=4800, 9600 linear taps, 4800 minimum-phase taps. firwin2 default mesh=16385, inverse length32768. |
| AutoEQ `:859–900,1033–1105,1241–1305` | [V-local] k=1 default interpolation/extrapolation; SG degree2, repeated normal/treble smoothing, expit blend; **k=2 FITPACK** to reconstruct removed clipping-kink samples. | Frequencies can number approximately 6,000 before log resampling. Target zero frequency temporarily becomes .001 Hz, restored after evaluation; low-frequency gains separately held constant for FIR generation. |
| AutoEQ `:1447–1489`; `biquad.py:22–131` | [V-local] linregress slope, std ddof1; RBJ magnitude algebra; feedback coefficients have a sign convention reversed again by digital_coeffs. | Normal fit casts f64, but `_init_data` preserves numeric input dtype; ten-band initialization explicitly creates float32 arrays then optimizer casts them. `biquad.impulse_response` is unimplemented and unused. |
| plotting `impulse_response_plotter.py:114–222,459–575` | [V-local] Spectrogram PSD with periodic Hann, adaptive segment length; waterfall magnitude spectrogram; log-axis k1 interpolation; **2D uniform_filter(size=3,mode='constant')**. | PSD nperseg typically4800, approximately200 frames. Waterfall takes first1792 samples, nperseg256, overlap128: 129×13 before dropping DC; log grid≈263×13, then cropped. Uniform filtering is zero-padded, not reflect. |
| plotting `analysis.py:103–138` | [V-local] Correlate + correlation_lags for IACF/IACC; explicit f64, energy normalization, ±1 ms lag window, abs-peak. | Different from signed early-onset correlation in HRIR; preserve both. |

**[V-local] Dtype qualification:** normal file-derived DSP is float64/complex128, but `ImpulseResponse` stores caller arrays without conversion and some cumulative operations preserve float32. Auxiliary installed NumPy 2.4.4 source allocates FFT output through `result_type(a.dtype,1j)`, whereas public NumPy FFT documentation describes float32 promotion; do not generalize either statement to every allowed NumPy version. The repository permits NumPy>=1.26, SciPy>=1.12 and nnresample>=0.2.4. Pin a concrete oracle environment and record actual dtypes. The inspected auxiliary environment is not proof of the running application's environment. [2,3,18]

### 2.3 AutoEQ optimizer: exact required problem, not just a solver name

**[V-local]** The parameter vector is grouped, not interleaved: x=[log10(fc_1..B), ln(Q_1..B), gain_1..B]. Bounds are log10(max(10,frequency[0])) to log10(fs/2); ln(.1) to ln(20); gain -60 to +60 dB. Fixed-band mode fits only B gains with the same gain bounds. Residual r_i=sum_j biquad_dB(f_i,fc_j,Q_j,g_j)-target_i. The scalar objective is 0.5*sum(r_i²), but the solver must receive the **M-vector residual**, not SSE as one residual. [1]

**[V-local]** Initialization is substantive: 1/7-octave SG smoothing repeated 1,000 times; positive/negative peaks; discard |gain|<=.1; optionally add 20/60-Hz initial filters; merge same-sign neighboring peaks when line-fit RMSE<.3; filter-count reduction at .2/.33 dB; Q=1. Post-fit discard |gain|<=.1 and fc<=10 and sort by fc. Changing peak ordering, tie-breaking, smoothing endpoints or thresholds can change the optimization basin. Returned `rmse` is computed **before** post-fit filter removal, so independently measure final returned curve error. [1]

**[V]** The call omits solver/Jacobian options: default TRF, dense 2-point finite differences, linear loss, effective x_scale=1, ftol=xtol=gtol=1e-8. No sparsity pattern, robust loss or analytical Jacobian is supplied. Dense exact TRF uses a Jacobian SVD and bound-distance/reflection logic. max_nfev=max(20,int(max_time*120)); default max_time5 means **600 counted residual evaluations, not five seconds and not 600 total model calls**. Numerical-Jacobian evaluations are excluded from SciPy nfev; None delegates to the version-specific default (100*n in SciPy1.16.1). [4]

**[I]** M≈695, B=10 gives 695×30=20,850 doubles (166,800 bytes) for the dense Jacobian; small dense QR/SVD is appropriate. Sparse solvers and GPU infrastructure are unnecessary for this subproblem. Each iteration may require dozens of additional residual evaluations. Cancellation should be checked between model/Jacobian evaluations.

| Language | Verified implementation choices | Judgment / remaining work |
|---|---|---|
| C++ | [V] Ceres 2.2.0 has box bounds, DynamicNumericDiffCostFunction, dynamic M residuals and DENSE_QR. Eigen unsupported LM and GSL nonlinear LS do not provide the same box-constrained interface. [30–32] | [I] Best mature alternative. Ceres LM/Dogleg with bounded handling is **not SciPy TRF**. Its NumericDiff can probe outside bounds; supply bounds-aware FD or analytic derivatives. max_num_iterations is not max_nfev. |
| Rust | [V] argmin generic trust regions are not bounded residual TRF; levenberg-marquardt has no variable box bounds. basin1.7.0 supplies box constraints/Coleman–Li scaling but explicitly omits reflected steps and an explicit trust radius. trust-region-least-squares0.11.0 is **dense unbounded**, despite its SciPy-parity claims. [24,25] | [I] Trial basin on the corpus, or bind Ceres through a narrow native interface; do not claim either reproduces TRF. A young solver needs more verification than Ceres. |
| C# | [V] MathNet5.0 bounded LM accepts lowerBound/upperBound/scales/isFixed; NonlinearModel supports numerical Jacobians. Two-sided bounds use x=L+(U-L)(1+sin(u))/2. [40] | [I] Credible replacement problem, not identical solver. Bound derivatives vanish at endpoints; default central differences differ from SciPy forward differences and can probe out of bounds. Instrument actual calls; its counters are not SciPy's. |
| Go | [V] Gonum optimize offers scalar-objective BFGS/LBFGS/CG/Newton/Nelder–Mead, not an identified bounded dense residual TRF. [34] | [I] Bind Ceres or own a bounded least-squares implementation. Unconstrained LM plus clamp is not equivalent. Scalar minimization with a bounded transform is a different algorithm and must pass fit-quality tests. |
| JVM / Dart | [V] Commons Math LM/Gauss–Newton lack native constraints; ParameterValidator is a workaround. No corresponding production bounded TRF identified in inspected Dart packages. [45,46] | [I] Native solver binding or additional implementation; neither earns a parity exemption. |

**[I] Optimizer acceptance must measure response quality, not fc/Q/gain equality.** Different biquads can fit nearly identical curves. Start from identical x0 and target; require bound compliance, no nonfinite residual, final RMSE no worse than Python by max(.01 dB,1% of Python RMSE), and no more than .1 dB increase in maximum absolute target error. These are proposed engineering gates, not established psychoacoustic thresholds. Separately compare returned response curves and error distributions on a diverse corpus. If an alternate solver fails, retain the Python solver during migration or port TRF; do not loosen all downstream waveform thresholds to conceal it.

### 2.4 Algorithms that must be specified precisely

#### Homomorphic minimum phase

**[V] Reference algorithm pinned to SciPy1.17.1 source, consistent with inspected auxiliary source.** Let L=len(h), F=n_fft. Require F>=L. Default F is 2^ceil(log2(2*(L-1)/.01)); the application's call instead explicitly uses F=L, an even linear-FIR length. [5]

1. A=abs(FFT(h,F)); add **1e-7*min(A[A>0]) to every bin**, then log. This is neither max(A,epsilon) nor log(A+1e-12).
2. Default half=True: multiply log magnitude by .5. Compute real IFFT to obtain cepstrum.
3. Zero lifter of length F; w[0]=1; stop=F//2; w[1:stop]=2; **if F is odd, w[stop]=1**. As written, even F leaves its midpoint zero. Do not silently replace this with a textbook parity convention.
4. real(IFFT(exp(FFT(cepstrum*w)))) and retain ceil(L/2) samples; half=False retains L and skips the .5 multiplier.
5. All-zero spectra have no positive minimum; preserve/explicitly specify validation instead of inventing a floor.

**[V-local]** AutoEQ doubles dB gain before firwin2 because half=True targets square-root magnitude; then it forces Nyquist gain to zero. With 48k/f_res10, F=L=9600 and result length4800. Increasing F to the default 'more accurate' FFT size changes the algorithm. [1]

**[V/I] Version hazard:** tagged SciPy1.12 source uses a lifter stop based on input length; later source uses n_fft. For this application's even F=L call, that difference does not change the index; for general/default-F use it can. Export the actual oracle's intermediate log spectrum and lifter, not merely the name `minimum_phase`. half=False was added in1.14. [5,6]

#### firwin2

**[V]** For T taps, fs/2=Nyquist and unspecified nfreqs, mesh size Q=1+2^ceil(log2(T)), Q>T. Validate sorted frequencies starting at0 and ending at Nyquist. Interior duplicate frequencies may appear twice to encode a discontinuity; perturb the two copies by epsilon≈machine_epsilon*Nyquist and reject remaining collisions. Linear interpolation onto Q uniformly spaced frequencies precedes multiplication by exp(-i*(T-1)*pi*f/fs). Apply irfft with implicit length2*(Q-1), retain first T samples, multiply a **symmetric Hamming** window (`fftbins=False`). No final gain normalization. Symmetric even-tap TypeII requires zero gain at Nyquist; antisymmetric TypesIII/IV have additional endpoint constraints. [5]

**[I]** Implement the actual TypeII application path first, with explicit input validation; scope estimates do not cover a fully general SciPy clone. Frequency interpolation here is linear frequency, distinct from AutoEQ's prior log-frequency spline. Retain the separate grids. NWaves offers a related designer, not proven identical endpoint/discontinuity behavior.

#### nnresample and polyphase processing

**[V]** PyPI release0.2.4.1 dates to2021-01-11; inspected installed copies declare that version. Upstream master declares0.2.5 and differs, notably adding an immediate same-rate copy. Pin the release actually used. Both reviewed design paths use default N=32001 and attenuation60 dB, producing beta=.1102*(60-8.7)=**5.65326**. `compute_filt`'s direct default beta5 is not `resample`'s effective beta. [9–12]

**[V] Default null-on-Nyquist design:** reduce up/down by gcd; q=max(up,down). First design normalized low-pass `firwin(N,1/q,window=('kaiser',beta))`. Pad to F=2^19; calculate real FFT. Let H=F/2+1=262145, c=sqrt(1+(beta/pi)^2), bot=floor(H/q), top=ceil(H*(1/q+2*c/N)). Find minimum magnitude over [bot,top); f_null=(bot+argmin)/H; final cutoff=2/q-f_null; redesign with firwin. Note the denominator H, not F/2. FIR uses symmetric Kaiser-windowed sinc and unity DC scaling. Cache/filter search should be deterministic. [10,11]

**[V]** Parameter disambiguation derives beta from attenuation with Kaiser piecewise equations. Default N is constant32001, not automatically computed from every sample-rate ratio. The inspected optional df-to-N branch multiplies by df rather than algebraically inverting the attenuation formula; that suspicious branch is not exercised by the app's default call. Do not 'fix' it in a parity port without a distinct algorithm change. [11]

**[V] resample_poly with explicit float64 taps:** after gcd reduction, n_out=ceil(n_in*up/down); half_len=(N-1)//2; h*=up; pre_pad=down-half_len%down (a full down when divisible); pre_remove=(half_len+pre_pad)//down. Add the minimum post-padding necessary to yield at least n_out+pre_remove samples. Run upfirdn with default constant-zero signal extension, then keep [pre_remove:pre_remove+n_out]. For 48k→44.1k, up147/down160, half_len16000, pre_pad160, pre_remove101. Never allocate the literal upsampled signal; compute its polyphase equivalent. [12]

**[V/I]** libsamplerate, soxr, r8brain, rubato and NWaves are not evidence of this exact design/phase/cropping contract. Some provide excellent audio SRC; that is a different criterion. Preserve exported taps and implement a custom-tap polyphase kernel, then separately reproduce the null-search design. Installed0.2.4.1 lacks the new same-rate guard and can attempt invalid cutoff1; equal-rate policy must be decided explicitly. [10,26,29,39]

#### Savitzky–Golay, peaks, windows and interpolation

**[V] SG:** application degree2, deriv0, odd window computed by Python round(log(2^octaves)/log(mean_frequency_ratio)), then increment if even. SciPy default mode='interp' fits a degree2 polynomial to the first/last W input samples and evaluates the first/last W//2 output positions; it does not pad them with reflected/constant values. Polynomial coefficients/interior convolution and endpoint fits must both match. Input float32 is retained by SciPy; other non-float types become float64. At ratio1.01: 1/7 octave gives W11, 1/3 gives W23, 1/12 gives W7. Repeating1,000 times magnifies small systematic edge differences. [1,7]

**[V] peaks:** strict neighboring comparison, a flat-top run returns one midpoint (floor for even length), no array endpoints, height interval includes its bounds. The app needs this simple height/plateau subset, not all SciPy prominence/width machinery. Preserve ascending index order; its earliest peak is not the largest peak. NaNs need an explicit policy. [8]

**[V-local/V] windows/filtering:** symmetric Hann for fades and Hamming for firwin2; **periodic** Hann for named-window spectrograms. Kaiser requires Bessel I0 and correct N=0/1 behavior. Actual uniform_filter is separable 2D size3, constant zeros after dB conversion; cropping follows. A full reflect-mode implementation is not required by current usage. [2,13]

**[V] FITPACK interpolation:** InterpolatedUnivariateSpline is s=0, default ext=0 (polynomial extrapolation), not endpoint clamp. k1 is piecewise linear. For strictly increasing x, k3 is **not-a-knot**, not natural cubic. In 1-based indexing with m points: cubic interior knots x3..x(m-2), endpoints repeated4. For k2, interior knots (x2+x3)/2 through (x(m-2)+x(m-1))/2, endpoints repeated3. Fit B-spline collocation coefficients, evaluate with de Boor, and extrapolate endpoint polynomial pieces. [14,15]

**[V-local/I]** Use x=log10(f), not interpolation on Hz followed by a logarithmic graph. k2 is exercised by EQ kink smoothing; no literal active k3 call was found in requested processing files, but public `interpolate(pol_order=3)` makes it a compatibility requirement. Natural cubic fixes endpoint second derivatives to zero and generally changes endpoint and extrapolated gain. This can alter FIR coefficients and BRIR. Gonum's NotAKnotCubic helps inside the domain, but its endpoint-clamping evaluation still needs adaptation. [1,34]

#### FFT and numerical parity

**[V]** SciPy FFT uses pocketfft; RustFFT, Gonum FFTPACK and MathNet managed algorithms have different operation ordering. RustFFT/RealFFT and Gonum transforms need explicit inverse normalization; MathNet exposes normalization options. C++ pocketfft gives the same family of algorithms, not a guarantee of matching SciPy build/compiler/SIMD choices. Real inverse transforms require the original N, especially odd N; without it many APIs infer an even length. [16–18,19,27,34,38]

**[V-local]** Distinguish **legacy scipy.fftpack.next_fast_len (235-smooth)** in AutoEQ/tail trimming from **scipy.fft.next_fast_len** in mic analysis. The latter is backend/version-sensitive. A different trim length is an observable shape change, not acceptable FFT roundoff. [1,2,16]

**[I]** Relative error is ill-conditioned near FFT zeros; deep dB nulls exaggerate tiny absolute differences. Nonlinear minima, thresholded peak/knee choices and quantization are more dangerous than ordinary FFT roundoff. Float64 FFT compatibility should usually permit tight tolerances, but no universal final-BRIR bound follows from machine epsilon. Measure and diagnose stage-by-stage.

### 2.5 Primitive coverage matrix

**Legend:** L=verified library provides the operation; A=library exists but needs semantic adaptation; H=hand-written compatibility algorithm recommended/no suitable exact API verified. H does not claim the entire language ecosystem lacks implementations. Cell judgments are **[I]**, based on verified APIs in the candidate tables and sources. Numbers are **estimated production SLOC**, excluding tests and pipeline translation, for the limited app contract. All targets use double except explicitly identified rejected alternatives.

| Primitive family | Rust | C++ | Go | C#/.NET |
|---|---|---|---|---|
| Arrays, reductions, complex, QR/SVD | L ndarray/nalgebra | L Eigen | L Gonum mat | L MathNet |
| rFFT/irFFT/complex FFT, arbitrary odd lengths | A RustFFT/RealFFT, 40–90 | A pocketfft, 40–90 | A Gonum fourier, 40–100 | A MathNet, 40–100; not NWaves/FftSharp for odd sizes |
| Legacy235 and modern fast length | H 40–90 | H 40–90 | H 40–90 | H 40–90 |
| fftconvolve/convolve; correlation and lags | A sci-rs, 100–220 | H atop pocketfft, 120–250 | H atop Gonum, 140–280 | A NWaves64, 100–240 |
| Butterworth, ZPK/SOS and DF-II-T filtering | A sci-rs, 150–350 | A Iir1, 150–350 | H 300–650 | A NWaves design + FilterChain64, 180–380 |
| firwin2 exact app subset | H 120–220 | H 120–220 | H 140–260 | A NWaves DesignFilter.Fir, 100–220 |
| Homomorphic minimum phase | H 70–140 | H 70–140 | H 80–150 | H 80–150 |
| Savgol degree2, variable W, interp edges | A sci-rs coefficients/savgol-rs, 80–200 | H Eigen QR, 150–280 | H Gonum QR, 180–300 | H MathNet QR, 150–280; NWaves fixed SG insufficient |
| find_peaks height + plateaus | H 50–100 | H 50–100 | H 50–100 | H 50–100 |
| Hann/Hamming/Kaiser/I0 | A/H 80–170 | A/H 80–170 | A/H 90–180 | A/H NWaves/FftSharp, 60–150 |
| size3 constant-zero 2D uniform + running mean | H 40–80 | H 40–80 | H 40–80 | H 40–80 |
| Log interpolation k1, k2, k3/extrapolation | H nalgebra solve/de Boor, 250–650 | H Eigen solve/de Boor, 250–450 | A k1/k3 Gonum; H k2/extrap, 250–550 | A linear; H k2/k3, 280–550 |
| Bounded nonlinear residual fit | A basin or Ceres binding, 200–450, risky | A Ceres, 250–450 | H solver/binding, 450–1000 | A MathNet bounded LM, 250–500 |
| linregress consumed fields / expit | H regression; L statrs logistic, 30–80 | H 40–100 | L Gonum stat/logistic CDF, A20–70 | L MathNet Fit.Line/Logistic, A20–70 |
| Spectrogram PSD/magnitude/framing | H realfft, 140–300 | H pocketfft, 140–300 | H Gonum, 160–340 | H MathNet; NWaves STFT float-centric, 160–340 |
| nnresample design + custom-tap polyphase | H 250–500 | H 250–500 | H 300–600 | H 280–550 |
| RBJ + arbitrary-frequency freqz | A/H 100–230 | A/H Iir1/formulas, 100–230 | H 120–250 | H f64 formulas, 120–250 |

**[I]** Shared validation/buffer abstractions reduce totals; column sums are not delivery estimates. Handwritten FIR design, filter normalization, extrapolation and optimizer interfaces must be owned even if a library supplies most arithmetic. An entire generic SciPy function with all modes would cost much more. Implement banded rather than dense spline solves for thousands of points; the degree fixes narrow bandwidth.

### 2.6 Library evidence and status

#### Rust

| Candidate | Verified version/status as observed | Actual fit / caution |
|---|---|---|
| rustfft / realfft | [V] 6.4.1 (2025-09-18) /3.5.0 (2025-06-12). [19] | [V] f64 arbitrary complex/real lengths including odd; no automatic normalization. Good foundation. |
| ndarray / nalgebra | [V] .17.2 (2026-01-10) /.35.0 (2026-05-24). [20] | [V] Arrays versus dense decompositions; not SciPy signal. sci-rs uses older ndarray/nalgebra, LM another nalgebra version; pin compatible types or copy at boundaries. |
| sci-rs | [V] .4.1 (2024-11-22). [21] | [V] butter_dyn/ZPK/SOS/sosfilt; convolution/correlation and SG. Convolution uses power-of-two FFT; valid with first input shorter returns empty; SG pads nearest, not interp; Fourier resample is not polyphase. API is useful but parity must be tested. |
| idsp / biquad | [V] .22.1 (2026-08-04) /.6.0 (2026-03-22). [22] | [V] Embedded-oriented DSP versus generic f64 RBJ/DF1/DF2T. biquad shelves take Q, not directly shelf-slope S. Not a whole scipy.signal replacement. |
| rubato | [V] 5.0.0 (2026-08-10). [23] | [V] f64 SRC; current Fft/Async/Slip APIs, not old API names. No verified arbitrary FIR-array equivalent to resample_poly window. |
| argmin / levenberg-marquardt | [V] .11.0 (2025-09-28) /.15.0 (2025-08-03). [24] | [V] Neither verified as boxed SciPy TRF. LM stepbound is an initial step limit, not parameter bounds. |
| basin / trust-region-least-squares | [V] 1.7.0 (2026-08-28) /.11.0 (2026-08-31). [25] | [V] Former bounded LM-like Coleman–Li variant; latter dense **unbounded** parity project. [I] Too new to treat release activity as maturity. |
| splines / enterpolation | [V] 5.0.0 (2025-04-26) /.3.0 (2025-05-08). [26] | [V] Segment interpolation / B-spline curve construction. No verified FITPACK sample-interpolation coefficient solver; Catmull–Rom is not not-a-knot. |
| statrs / plotters | [V] .19.1 (2026-08-11) /.3.7 (2024-09-08). [26] | [V] Logistic/statistics versus PNG/SVG/log axes. No verified statrs linregress or I0; plotters does not compute spectrograms. |
| Additional small crates | [V] savgol-rs .1.0 offers interp edge fitting; find_peaks .1.5 exposes plateau midpoint but returns height order; sdr .7 firwin2 fixes512 gains; spectrograms2.1.3 offers f64 STFT/minphase. [26] | [I] References, not automatic compatibility approvals. Spectrograms minphase targets same magnitude/length, not SciPy half=True. |

**[I] Recommended Rust subset:** RealFFT/RustFFT, one chosen matrix representation, audited sci-rs Butterworth/SOS code, dedicated compatibility functions, and a separately replaceable optimizer. Do not introduce every listed crate. Publication dates do not establish maintainer responsiveness or future stability.

#### C++

| Candidate | Verified version/status as observed | Actual fit / license concern |
|---|---|---|
| Eigen | [V] Homepage stable5.0.0 dated2025-09-30; 5.0.1 docs also exist. MPL-2.0. [27] | [V] Double QR/SVD. FFT and LM are unsupported modules; default FFT backend KissFFT. [U] Latest release ambiguity not settled. |
| pocketfft | [V] C++11 header-only cpp branch, no numbered release observed; BSD-3-Clause. [27] | [V] f32/f64/long double, arbitrary sizes/Bluestein, explicit scaling. [I] Preferred FFT; pin commit. |
| FFTW | [V] 3.3.11 (2026-04-21), GPL-2.0-or-later or separate commercial license. [28] | [V] Arbitrary f64 transforms. GPL is a real distribution condition, not a technical blocker for GPL-compatible licensing. Unnecessary here. |
| KissFFT / PFFFT | [V] Kiss131.2.0 (2025-10-22), BSD; PFFFT marton78 fork has double APIs, no latest version settled. [28] | [V] Kiss defaults float unless configured double; optimized real API even-only. PFFFT fork is not arbitrary-length/odd-compatible despite double support. Distinguish original float-only project and current fork. |
| KFR | [V] 7.1.0 (2026-08-18), GPL-2.0-or-later/commercial. [28] | [V] f64 DFT/DSP/SOS/SRC; general dft_plan versus power-of-two ngfft_plan. [U] Exact odd-real API restrictions not settled. |
| JUCE dsp | [V] 9.0.1 (2026-08-10), AGPLv3/commercial. [28] | [V] Some double templates, but public FFT is float and 2^order. [I] Bad FFT foundation for this workload; owning JUCE does not eliminate a second FFT backend. |
| DSPFilters / Iir1 | [V] DSPFilters has no published releases observed, MIT; Iir1 1.10.0 (2025-07-06), MIT. [29] | [V] Iir1 double state/input/output, Butterworth/RBJ, external SOS and SciPy-comparison tests. [U] DSPFilters current maintenance not settled. Section ordering still needs parity tests. |
| libsamplerate / soxr / r8brain | [V] .2.2 (2021-09-05), BSD2; .1.3 (2018-02-24), LGPL2.1+; r8brain source7.5, MIT. [29] | [V] libsamplerate public sample buffers float; soxr supports f64 and separate internal-double option; r8brain double, optional PFFFT settings can reduce precision. No verified exact nnresample-taps contract. Old release date alone does not prove abandonment. |
| Ceres / Eigen LM / GSL | [V] Ceres2.2.0 stable in history (2.3 release not verified), BSD3, C++17; Eigen unsupported LM MPL2; GSL2.8 (2024-05), GPL3+. [30–32] | [V] Ceres supplies boxes. Eigen LM and GSL NLS are not boxed TRF. GSL cspline is natural, not FITPACK not-a-knot. |
| Armadillo / xtensor | [V] Armadillo15.4.2, Apache2; xtensor .27.1, BSD3, .27 requires C++20. Dates not settled. [33] | [V] Arrays/linear algebra/FFT-conv options do not solve FITPACK/TRF semantics. xtensor-blas lstsq is linear LS; BSD xtensor-fftw wrapper still links GPL FFTW. [I] Avoid redundant matrix stacks. |

#### Go and C#/.NET

| Candidate | Verified version/status as observed | Actual fit / caution |
|---|---|---|
| Gonum | [V] v.17.0 (2025-12-29) observed; BSD3; latest-status ambiguity noted. [34] | [V] f64 FFTPACK real/complex arbitrary lengths, mat QR/SVD, stat, linear and NotAKnotCubic interpolation; evaluator clamps outside domain. No identified boxed residual solver or complete signal-design stack. |
| mjibson/go-dsp | [V] pseudo-version 20260128111154-6db759bd4208 (2026-01-28), repository redirects to madelynnblue/go-dsp, ISC. [35] | [V] FFT/windows/Pwelch. [I] Calling it unchanged since the2010s would be wrong; it still is not scipy.signal. |
| go-audio | [V] audio v1.0.0 (2018-10-13); archived2026-02-21; Apache2. [35] | [V] FloatBuffer uses []float64; primarily buffers/I/O infrastructure, not a maintained DSP substitute. |
| zaf/resample | [V] 1.5.0 (2024-01-03), BSD wrapper, native libsoxr dependency. [36] | [V] F64 exists; cgo/pkg-config/library distribution remains. Not exact custom Kaiser resample_poly. |
| scientificgo | [V] fft .0.1 (2025-03-13), arbitrary complex128 FFT; special .0.0 (2020-03-16); BSD3. [37] | [U] No corresponding complete optimizer, Kaiser I0 or expit API established. [I] Not a reason to choose Go over Gonum. |
| MathNet.Numerics | [V] stable5.0.0 (2022-04-03), 6.0.0-beta2 (2025-03-02), MIT. [38,40] | [V] Managed arbitrary-length FFT uses Bluestein; real transform packing/scaling must be wrapped. Actual bounded LM. Stable-release age is a maintenance concern, not proof of abandonment. |
| NWaves | [V] .9.6 (2021-10-06), MIT; recent commit activity not settled. [39] | [V] Fft64/RealFft64, FirFilter64/IirFilter64/Ola/Ols64 exist, but FFT is power-of-two. DesignFilter.Fir explicitly compares to firwin2. Generic STFT/resampling/RBJ classes often use float32. SG has fixed small windows, not SciPy interp. |
| FftSharp | [V] 2.2.0 (2024-11-16), MIT. [41] | [V] Useful FFT/window helpers, Kaiser; power-of-two FFT, not full DSP/optimization. |
| Accord.NET | [V] 3.8.0 (NuGet2017-10-19); archived2020-11-19; LGPL2.1. [42] | [I] Reject as the principal dependency for a new port. Author willingness to relicense is not a blanket MIT license. |

**[I]** C# can use MathNet FFT/linear algebra/LM and carefully selected NWaves coefficient-design code. Do not route double EQ evaluation through NWaves PeakFilter: reviewed source computes then casts coefficients to float. Go should start with Gonum and purpose-built compatibility functions, not stack abandoned audio packages and assume completeness.

#### Kotlin/JVM and Dart (short assessment)

**[V] JVM:** JTransforms README3.2, BSD2, DoubleFFT_1D supports arbitrary positive lengths with mixed radix/Bluestein. KMath has experimental interpolation/optimization modules; Multik supplies arrays/linear algebra, both Apache2. Commons Math LS documents no native constraints. **[I]** Kotlin can obtain reliable arithmetic through Java but still needs firwin2/minphase/SG edges/FITPACK k2/polyphase/solver compatibility. No DSP advantage over C++/Rust/C# demonstrated. [43–45]

**[V] Dart:** fftea1.5.0+1 (Apache2) supplies Float64x2List FFT, arbitrary lengths, STFT and convolution. SciDart .0.2-dev.12 (Apache2, experimental) includes firwin, lfilter, Kaiser, conv/correlation and peaks; **firwin is not firwin2**. Its peaks test neighbor <=/>= and can select multiple flat points, unlike SciPy midpoint plateaus. **[U]** No suitable bounded TRF/FITPACK k2/SG interp/minphase/exact custom polyphase combination verified. **[I]** Reject Dart as the primary numeric core unless a shared native engine is used. [46]

### 2.7 DSP gap scores and ranked decision

**[I] Definition:** score counts unresolved **semantic families**, not packages. Fixed20 families: FFT; fast length; convolution; correlation/lags; Butterworth/SOS; firwin2; minimum_phase; SG; peaks; windows; uniform; linear interpolation; quadratic FITPACK; cubic FITPACK; bounded fit; regression; expit; spectrogram; exact resampling; RBJ/freqz. C means a custom algorithm/adapter is required; R means usable library exists but precision, boundary or fit equivalence remains risky. Higher C+R means more contracts to own. Arrays are excluded because all four principal languages have adequate options.

| Rank | Language | C /20 | R /20 | C+R | Estimated production compatibility SLOC | Principal disadvantage |
|---|---|---:|---:|---:|---:|---|
| 1 | C++ | 13 | 5 | 18 | 2,000–4,000 | Build/ABI and license discipline; mature solver still not TRF |
| 2 | Rust | 12 | 6 | 18 | 1,800–3,800 | Young bounded-solver choice, smaller SciPy-compatibility ecosystem |
| 3 | C# | 11 | 6 | 17 | 2,000–4,200 | Mixed float/double DSP, aging stable distributions, spline work |
| 4 | Go | 13 | 5 | 18 | 2,800–5,200 | Missing bounded residual solver and filter-design layer |
| 5 | Kotlin/JVM | not individually scored | not individually scored | approximately18–20 | 3,000–5,500 | Additional API verification needed; no superior compatibility package found |
| 6 | Dart | not individually scored | not individually scored | approximately19–20 | 3,500–6,000+ | Experimental signal stack and the largest unverified semantic surface |

**[I] Counts are planning classifications, not measured defect counts.** Lower C# count does not outrank C++: MathNet/NWaves offer more named functions but C++ has mature double numerical kernels, a stronger bounded-solver option and a direct pocketfft path. Tiny uniform/expit helpers count as one just like a difficult resampler, so SLOC/risk weighting is more meaningful than raw totals. Rust's estimated code volume can be lower while its verification burden is higher. Add roughly **1,000–3,000+ lines and substantial numerical testing** if a SciPy-like bounded TRF itself must be owned; exact costs remain unknown. Pipeline algorithms, parsers, per-speaker scheduling and the existing23k Python rewrite are **not included**.

**[I] Stack consequences:** S2 gains the best native DSP ecosystem from C++, not from Electron. S1 has a viable Rust engine with explicit compatibility ownership. S3's Wails frontend simplicity does not erase Go's DSP gaps. A C++ numeric library behind Rust, C# or Go is also a valid unlisted architecture, but it introduces FFI packaging; native dependency-free rhetoric no longer applies. Keeping Python as the first engine is the lowest numerical-risk migration, even if it does not immediately remove Python distribution costs.

## 3 Proposed architecture

### 3.1 A versioned numeric engine, independent of shell

**[I]** Use four components: (1) plain typed configuration and channel/sample buffers; (2) numerical primitives with explicit contracts; (3) pure per-speaker stages and deterministic assembly; (4) separate numeric-analysis data and renderers. GUI/CLI invoke the same engine, but this does **not** propose rerouting the existing CTk frontend through the WebView JSON service; ADR0001 remains respected. [3]

**[I] Recommended C++ engine:** pocketfft pinned commit; Eigen pinned compatible release; Iir1 only where coefficient/time-response fixtures pass; custom compatibility helpers; Ceres as a pluggable optimizer. Expose simple C ABI functions for run/cancel/progress plus a standalone headless CLI. No GUI types, managed exceptions, STL objects or ownership ambiguity across FFI. Rust may implement the same interface with RustFFT/RealFFT and nalgebra. Run speaker tasks with independent state and deterministic final reduction order; do not assume 'compiled language' automatically means thread-safe caches or free parallel speedup.

**[I]** Pin semantics, not incidental library APIs: transform normalization and original length; inclusive/exclusive crop indices; dtype and channel order; spline degree/knots/extrapolation; exact filter taps; SOS state/ordering; residual vector and bounds; cancellation/evaluation accounting. First migrate stages while comparing against Python. Retain Python's optimizer behind a narrow adapter if replacement quality has not passed. A future Python binding to the native core could preserve pip-install use; wheel/toolchain feasibility is outside this dimension and not asserted.

**[I]** Plot tests consume numeric arrays before rendering. PNG pixels will differ across fonts/renderers and should not be a BRIR gate. Validate PSD, magnitude, ILD/IPD/IACC arrays separately; image snapshots can test missing labels, clipping and legends. Interactive HTML library selection belongs to the UI/output research.

### 3.2 Concrete parity test plan

**[I] Stage0: freeze the oracle.** Select a known-good commit and exact Python/NumPy/SciPy/nnresample/soundfile versions on the same reference machine; record CPU, OS, BLAS/LAPACK, compiler/backend and thread limits. Capture actual runtime versions/dtypes, not the auxiliary environment inspected here. Save raw little-endian float64/complex128 buffers plus JSON shape/units/channel metadata (or .npy with a validated loader). SHA-256 these fixtures and inputs. Export intermediate arrays before quantization and before plotting. No fixtures were generated during this research.

**[I] Stage1: isolate algorithms with golden inputs.** Feed every new primitive the Python stage's saved input, not the new upstream output, so a failure identifies that primitive. Then run chained differential tests. Maintain both tests; otherwise later stages hide upstream errors.

| Golden set | Required cases | Initial proposed gate (not measured) |
|---|---|---|
| FFT/conv/correlation | Impulse, random fixed seed, sweep, zeros; N1/2/8/9/1024/1025/48000/48001, sweep295270, full590539; unequal sizes and even FIRs | Exact shapes/lags/crops. Complex comparison rtol1e-11, atol1e-12*max(1,input L1 norm); convolution atol1e-11*max(1,||x||2||h||2). Also relative L2 error<=1e-11 when nonzero. |
| Sweeps/inverse | fs44.1/48/96k, M1/2 and long sweep; PCM_32 roundtrip; custom odd-length WAV | Exact sample count/grid/segment indices. Prequantized peak-normalized max error<=1e-9 initially; inspect libm-dependent deviations. Quantization tested separately. |
| Windows/biquad/freqz/SOS | Low cutoff15Hz, crossover250Hz, shelves, Q .1/1/20, gains±60, near-Nyquist; impulses/noise/DC | Coefficients rtol1e-11/atol1e-13 when same representation; if SOS ordering differs compare complex response/time output instead. SOS output rtol1e-9, atol1e-11*reference peak scale. No unstable poles/nonfinite outputs. |
| Interpolation | k1/k2/k3, irregular log grids, minimal legal points, knots/endpoints, .001Hz substitute, extrapolation, invalid duplicates | Same knot topology/shape and rejection policy. In dB, atol1e-9 plus rtol1e-11. Natural cubic must fail targeted not-a-knot fixture. |
| SG/peak selection | Constant/linear/quadratic curves, edges, W7/11/23 and large windows, 1000 iterations; positive/negative plateaus, exact threshold, endpoints, silence | One SG pass atol1e-10 dB; 1000 passes atol1e-7 dB initially. Peak indices **exact** for nondegenerate fixtures; reject smoothing-induced threshold drift rather than hiding in waveform tolerance. |
| FIR/minphase | Flat/EQ/notched curves, even TypeII; export mesh, Hamming FIR, log magnitude, lifter, final FIR; explicit F=L and general odd cases | FIR coefficient rtol1e-9, atol1e-11*max(1,peak). Per-stage spectral difference<=1e-5 dB above relative -100dB floor. Deep nulls use absolute spectral error. |
| Resampling | 48↔44.1,48↔96,96↔44.1; short and odd N, impulses at start/end/interior, DC, tones near transition | Exact N_out, prepad/removal, onset. Stored-tap kernel rtol1e-10, atol1e-11*max(1,input peak); newly designed taps separately tested. Same null-search bin required unless explicit algorithm revision. |
| Decay/crop/ILD/IACC | Synthetic known decay/noise, delayed ears, ambiguous double peaks, silence/short records, real room data | Exact knee/crop/lag choices on well-separated fixtures; float regression outputs rtol1e-8/atol1e-10 in stated units. Mark threshold-degenerate cases and test policy explicitly. |
| Spectrogram/waterfall | Constant, tone, impulse, short recording; PSD and magnitude; zero-padded uniform filter | Exact axes/shape/frame policy; linear arrays rtol1e-9, normalized absolute1e-12. DC detrend, density normalization and one-sided PSD doubling tested independently. |
| Optimizer | Flat target/no filters; narrow/merged peaks; positive/negative bounds; fixed-band; B1/5/10/20; real calibration sets | Exact x0/residual before solver, separate quality gate from §2.3. Count model/Jacobian calls and failures. Never require identical filter parameters from different algorithms. |

**[I] Tolerance policy:** allclose near zero is insufficient; pair normalized absolute error with relative L2, finite masks and discrete-decision tests. Store units and scale in every fixture. Thresholds above are starting engineering budgets, intentionally tighter than final output budgets. Tighten from measured differences on Windows/macOS/Linux; do not silently widen after a failure. Golden changes require an algorithm-change explanation. Bitwise comparison can remain for pure index operations, stored coefficients and deterministic same-binary repeated runs.

**[I] Stage2: final waveform gate for mathematically equivalent ports.** Use existing demo default/headphone-correction and virtual-bass250 configurations, then additional real measurement sets: room correction average/conservative, microphone correction, EQ files, all decay choices, all balance modes, cross-rate output, 1/2/7/16 speakers, inverted/unequal/silent/short inputs. Compare pre-write float64 output, then decode delivered WAV and compare again.

- Exact sample rate, track order/count, length, missing-channel zeros and finite/nonfinite policy. No independent normalization, time shift, resampling or gain fitting allowed before the strict comparison.
- Let p=max(abs(reference)), s=max(p,1e-12), e=new-reference. Proposed gate: **max|e|/s<=1e-6 and RMS(e)/max(RMS(reference),1e-12)<=1e-7** per non-silent channel. In near-silent channels require absolute full-scale max error<=1e-10 instead of dividing by tiny energy. Report both normalized and full-scale errors.
- On a common FFT grid, aggregate linear power into 1/12-octave bands from20Hz to min(20k,Nyquist). Compare 10log10(P_new/P_ref). Require **maximum .01dB band error** where reference band power is at least -80dB relative to its peak band; use absolute energy-difference limits below that floor. Also report unsmoothed spectral-error distribution above -100dB; band averaging must not hide narrow notches.
- **ITD: zero integer-sample onset/cross-correlation offset difference** for strict parity on well-defined peaks, each ear relative to reference and left-right difference. A one-sample shift is 20.83µs at48k and should be investigated, not normalized away. Report sub-sample correlation estimates diagnostically with proposed <=.05-sample drift on nondegenerate fixtures; do not confuse interpolation precision with physical accuracy.
- Report left-right band ILD difference<=.01dB; compare complex cross-spectrum/IPD only where both channels have sufficient energy, with proposed .1-degree tolerance. Compute IACC and its lag separately. These guard binaural changes that single-channel magnitude misses.

**[I] Stage3: optimizer replacement is a declared algorithm revision.** Evaluate its final response-quality gate separately; retain a strict render test that feeds **Python's frozen filter coefficients** through the new convolution pipeline. If the alternate solver returns acoustically equivalent but numerically different filters, the strict waveform gate may fail legitimately. Do not relax it globally. Approve the solver change with its own corpus, response differences, bound/convergence results and binaural timing analysis; create new algorithm-version goldens only after review. A uniform .1dB band tolerance is not enough to approve arbitrary waveform/phase changes.

**[I] Stage4: retire SHA gate deliberately.** Run old hash checks on untouched Python plus both new tolerance gates during migration. Inject known defects (one-sample delay, 0.02dB gain, Nyquist-bin error, natural-for-k2 spline, nearest-for-interp SG, wrong polyphase crop, float32 intermediate) and demonstrate the relevant tests fail. Only then replace the cross-language hash comparison; retain fixture hashes, artifact hashes and same-implementation determinism checks. Rollout needs multiple real measurements, not only one demo.

## 4 Risks ranked

| Rank | Risk | Consequence / mitigation |
|---|---|---|
| 1 | [V/I] Incorrect reference specification | Current dependency ranges, nnresample release/master drift and PCM_32 versus FLOAT32 can invalidate the entire comparison. Pin runtime and export real dtypes/formats first. |
| 2 | [V/I] Discrete changes from tiny numeric differences | Peaks, knee/crop lengths, filter count and FFT-fast trim can change by samples or whole filters. Exact decision fixtures before broad waveform tests. |
| 3 | [V/I] Solver substitution and out-of-bounds finite differences | Different minima, quality, stopping budgets and invalid evaluations. Bounds-aware residual/Jacobian adapter; quality corpus; Python fallback until approved. |
| 4 | [V/I] k2/k3 boundary/extrapolation or SG endpoint mismatch | Smooth-looking curves can generate different correction FIRs. Explicit FITPACK knot rules and 1000-pass endpoint goldens. |
| 5 | [V/I] SRC coefficient/phase/trim mismatch | 'High-quality' replacement shifts onset or changes transition response. Separate exact-tap polyphase test from design test; include very short signals. |
| 6 | [V/I] Silent precision and backend change | JUCE FFT/libsamplerate/NWaves common APIs may reduce precision; FMA/SIMD and reduction order alter bits. Audit actual types and disable aggressive fast-math in parity builds. |
| 7 | [V/I] Licensing/native dependency accumulation | FFTW/KFR/JUCE/GSL and soxr impose differing obligations; wrappers do not erase them. Select permissive core dependencies and review redistribution before adoption. |
| 8 | [I] False confidence from broad tolerances or one demo | Small spectral averages hide timing/phase defects. Test injected failures, per-stage arrays, binaural metrics and real-data corpus. |
| 9 | [V/I] Maintenance concentration | Young Rust solvers, old .NET stable packages, archived Go audio/Accord create ownership risk. Vendor only small audited code with attribution; budget maintenance. |
| 10 | [I] Overengineering | A generic SciPy clone, sparse infrastructure or total rendering rewrite before core parity wastes solo-maintainer effort. Implement the actual f64 contracts, then expand. |

## 5 Open questions you could not settle

- **[U] Actual shipping oracle environment:** inspected auxiliary SciPy1.17.1/NumPy2.4.4/nnresample0.2.4.1 is not proof of the environment used for current master CI hashes or packaged applications. Exact installed builds must be captured at export.
- **[U] Final tolerance attainability:** no new native engine, golden exports, benchmark or cross-platform numerical measurements ran. Proposed error budgets require empirical qualification.
- **[U] Optimizer operational reach:** repository search found optimizer calls within AutoEQ APIs/process, not a direct BRIR-pipeline invocation outside that module. Public API/R2 still requires it; confirm whether it is release-critical for normal BRIR generation or an ancillary feature before making it the first milestone.
- **[U] Output policy:** should the new engine preserve integer PCM_32 or introduce desired IEEE FLOAT32? Decide independently of numerical tolerance; preserve PCM-roundtrip sweep quantization unless explicitly changed.
- **[U] Existing defects versus intended semantics:** whole-Hann room mask, same-versus-full microphone APIs, gain-only long FIRs, equal-rate nnresample failure and decay timestamp conventions should not be silently corrected during parity work. Maintainer decisions are needed for intentional revisions.
- **[U] Numerical library contracts not executed:** sci-rs Butterworth/SOS equivalence, NWaves firwin2 discontinuities, Gonum tiny-N not-a-knot behavior, MathNet bound-transform convergence and basin fit quality remain untested.
- **[U] Latest release details:** Eigen homepage/docs disagree; Ceres2.3 stable not verified; PFFFT/pocketfft use commit-oriented sources; some project activity/release dates remain unknown. Pin selected commits rather than relying on this table indefinitely.
- **[U] Maintenance quality and support:** release recency is not a review of issue response, bus factor or security posture. Licensing notes are dependency-selection warnings, not legal advice.
- **[U] Workload tails:** production input-duration distribution, maximum actual FIR length, pathological EQ files and real 16-speaker memory footprint were not measured.
- **[U] Same-language determinism:** parallel reduction behavior and CPU-specific transcendental/FFT implementation may prevent byte identity even after a successful port. Establish per-platform deterministic expectations explicitly.

## 6 Sources

All accessed **2026-09-07**. Grouped source entries below contain multiple primary URLs where a package's API, release and implementation require separate evidence. Repository citations refer to revision0144dcc1f8a4634607b980d15c64549d4aa6c145; local readings, not web fetch summaries, establish the inventory.

1. AutoEQ source: [frequency_response.py](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/autoeq/frequency_response.py), [biquad.py](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/autoeq/biquad.py), [constants](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/autoeq/constants.py).
2. Core source directory and linked filenames in inventory: [core at audited commit](https://github.com/115dkk/Impulcifer-pip313/tree/0144dcc1f8a4634607b980d15c64549d4aa6c145/core), [audio_io](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/core/audio_io.py), [plotter](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/core/plotting/impulse_response_plotter.py), [analysis](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/core/plotting/analysis.py).
3. Project contracts: [pyproject](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/pyproject.toml), [magnitude parity tests](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/tests/test_magnitude_response_parity.py), [ADR0001](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/docs/adr/0001-native-frontends-stay-native.md).
4. SciPy [least_squares1.16.1](https://docs.scipy.org/doc/scipy-1.16.1/reference/generated/scipy.optimize.least_squares.html), [bounds-aware numerical differentiation](https://raw.githubusercontent.com/scipy/scipy/v1.16.2/scipy/optimize/_numdiff.py).
5. SciPy [FIR/minimum-phase1.17.1 source](https://raw.githubusercontent.com/scipy/scipy/v1.17.1/scipy/signal/_fir_filter_design.py), [minimum_phase documentation1.16.1](https://docs.scipy.org/doc/scipy-1.16.1/reference/generated/scipy.signal.minimum_phase.html).
6. SciPy [FIR source1.12](https://raw.githubusercontent.com/scipy/scipy/v1.12.0/scipy/signal/_fir_filter_design.py), [FIR source1.14](https://raw.githubusercontent.com/scipy/scipy/v1.14.0/scipy/signal/_fir_filter_design.py).
7. SciPy [savgol_filter](https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.savgol_filter.html).
8. SciPy [find_peaks](https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.find_peaks.html).
9. nnresample [PyPI release](https://pypi.org/project/nnresample/), [upstream](https://github.com/jthiem/nnresample), [master version](https://raw.githubusercontent.com/jthiem/nnresample/master/nnresample/__init__.py).
10. nnresample [resample/design source](https://raw.githubusercontent.com/jthiem/nnresample/master/nnresample/nnresample.py).
11. nnresample [parameter utility source](https://raw.githubusercontent.com/jthiem/nnresample/master/nnresample/utility.py), [dependency/license metadata](https://raw.githubusercontent.com/jthiem/nnresample/master/setup.py).
12. SciPy [resample_poly](https://docs.scipy.org/doc/scipy-1.16.1/reference/generated/scipy.signal.resample_poly.html), [1.17.1 signaltools source](https://raw.githubusercontent.com/scipy/scipy/v1.17.1/scipy/signal/_signaltools.py), [upfirdn source](https://raw.githubusercontent.com/scipy/scipy/v1.17.1/scipy/signal/_upfirdn.py).
13. SciPy [uniform_filter](https://docs.scipy.org/doc/scipy/reference/generated/scipy.ndimage.uniform_filter.html), [Kaiser](https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.windows.kaiser.html), [get_window](https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.windows.get_window.html).
14. SciPy [FITPACK1.17.1 fpcurf](https://raw.githubusercontent.com/scipy/scipy/v1.17.1/scipy/interpolate/fitpack/fpcurf.f), [FITPACK wrapper1.16.1](https://raw.githubusercontent.com/scipy/scipy/v1.16.1/scipy/interpolate/_fitpack2.py).
15. SciPy [smoothing spline tutorial](https://docs.scipy.org/doc/scipy-1.15.2/tutorial/interpolate/smoothing_splines.html), [B-spline implementation](https://raw.githubusercontent.com/scipy/scipy/v1.16.2/scipy/interpolate/_bsplines.py), [make_interp_spline](https://docs.scipy.org/doc/scipy/reference/generated/scipy.interpolate.make_interp_spline.html).
16. SciPy [legacy235 fast length](https://docs.scipy.org/doc/scipy/reference/generated/scipy.fftpack.next_fast_len.html), [modern fast length](https://docs.scipy.org/doc/scipy/reference/generated/scipy.fft.next_fast_len.html).
17. SciPy [pocketfft introduction/release1.4](https://docs.scipy.org/doc/scipy-1.17.0/release/1.4.0-notes.html), [spectrogram1.16.1](https://docs.scipy.org/doc/scipy-1.16.1/reference/generated/scipy.signal.spectrogram.html), [spectral scaling source1.17.1](https://raw.githubusercontent.com/scipy/scipy/v1.17.1/scipy/signal/_spectral_py.py).
18. NumPy [FFT documentation](https://numpy.org/doc/stable/reference/routines.fft.html). Dtype caveat in report is grounded additionally in the locally inspected auxiliary NumPy2.4.4 `_pocketfft.py:58–101`, not a claim about all NumPy releases.
19. Rust [rustfft6.4.1](https://docs.rs/crate/rustfft/6.4.1), [realfft3.5.0](https://docs.rs/crate/realfft/3.5.0), [RealFFT normalization/API](https://docs.rs/realfft/3.5.0/realfft/).
20. Rust [ndarray.17.2](https://docs.rs/crate/ndarray/0.17.2), [nalgebra.35](https://docs.rs/crate/nalgebra/0.35.0), [decompositions](https://docs.rs/nalgebra/0.35.0/nalgebra/linalg/index.html).
21. [sci-rs.4.1](https://docs.rs/crate/sci-rs/0.4.1), [API inventory](https://docs.rs/sci-rs/0.4.1/sci_rs/all.html), [convolution source](https://docs.rs/sci-rs/0.4.1/src/sci_rs/signal/convolve.rs.html), [SG source](https://docs.rs/sci-rs/0.4.1/src/sci_rs/signal/filter/savgol_filter.rs.html), [Butterworth source](https://docs.rs/sci-rs/0.4.1/src/sci_rs/signal/filter/design/butter.rs.html).
22. [idsp.22.1](https://docs.rs/crate/idsp/0.22.1), [biquad.6](https://docs.rs/crate/biquad/0.6.0), [biquad coefficient source](https://docs.rs/biquad/0.6.0/src/biquad/coefficients.rs.html).
23. [rubato5](https://docs.rs/crate/rubato/5.0.0), [Async API](https://docs.rs/rubato/5.0.0/rubato/struct.Async.html), [window enum](https://docs.rs/rubato/5.0.0/rubato/enum.WindowFunction.html).
24. [argmin.11](https://docs.rs/crate/argmin/0.11.0), [GaussNewton source](https://docs.rs/argmin/0.11.0/src/argmin/solver/gaussnewton/gaussnewton_method.rs.html), [LM.15](https://docs.rs/crate/levenberg-marquardt/0.15.0), [LM problem API](https://docs.rs/levenberg-marquardt/0.15.0/levenberg_marquardt/trait.LeastSquaresProblem.html).
25. [basin1.7 TRF source](https://docs.rs/basin/1.7.0/src/basin/solver/trf.rs.html), [trust-region-least-squares.11](https://docs.rs/crate/trust-region-least-squares/0.11.0).
26. [splines5](https://docs.rs/crate/splines/5.0.0), [enterpolation.3](https://docs.rs/crate/enterpolation/0.3.0), [statrs.19.1](https://docs.rs/crate/statrs/0.19.1), [plotters.3.7](https://docs.rs/crate/plotters/0.3.7), [savgol-rs source](https://docs.rs/savgol-rs/0.1.0/src/savgol_rs/lib.rs.html), [peak midpoint](https://docs.rs/find_peaks/0.1.5/find_peaks/struct.Peak.html), [sdr firwin2](https://docs.rs/sdr/0.7.0/src/sdr/fir.rs.html), [spectrograms minphase](https://docs.rs/spectrograms/2.1.3/src/spectrograms/min_phase.rs.html).
27. [Eigen](https://libeigen.gitlab.io/), [Eigen FFT](https://libeigen.gitlab.io/eigen/docs-5.0.1/unsupported/group__FFT__Module.html), [pocketfft C++](https://github.com/mreineck/pocketfft/blob/cpp/README.md).
28. [FFTW download](https://fftw.org/download.html), [release dates](https://fftw.org/release-notes.html), [FFTW license](https://www.fftw.org/doc/License-and-Copyright.html), [KissFFT](https://github.com/mborgerding/kissfft), [Kiss release](https://api.github.com/repos/mborgerding/kissfft/releases/latest), [PFFFT fork](https://github.com/marton78/pffft), [double header](https://github.com/marton78/pffft/blob/master/include/pffft/pffft_double.h), [KFR7.1](https://github.com/kfrlib/kfr/blob/7.1.0/README.md), [KFR DFT](https://docs.kfr.dev/dft/dft_introduction/), [KFR release](https://api.github.com/repos/kfrlib/kfr/releases/latest), [JUCE release](https://api.github.com/repos/juce-framework/JUCE/releases/latest), [JUCE9 license](https://github.com/juce-framework/JUCE/blob/9.0.1/LICENSE.md), [JUCE FFT](https://docs.juce.com/master/classjuce_1_1dsp_1_1FFT.html).
29. [DSPFilters](https://github.com/vinniefalco/DSPFilters), [Iir1](https://github.com/berndporr/iir1), [Iir1 release](https://api.github.com/repos/berndporr/iir1/releases/latest), [double state](https://raw.githubusercontent.com/berndporr/iir1/master/iir/State.h), [libsamplerate](https://github.com/libsndfile/libsamplerate), [float API](https://libsndfile.github.io/libsamplerate/api.html), [soxr releases](https://sourceforge.net/projects/soxr/files/), [soxr header](https://github.com/chirlu/soxr/blob/master/src/soxr.h), [r8brain](https://github.com/avaneev/r8brain-free-src), [r8brain version/precision](https://raw.githubusercontent.com/avaneev/r8brain-free-src/master/r8bbase.h).
30. Ceres [history](https://ceres-solver.readthedocs.io/latest/version_history.html), [license](https://ceres-solver.readthedocs.io/latest/license.html), [modeling](https://ceres-solver.readthedocs.io/latest/nnls_modeling.html), [solver](https://ceres-solver.readthedocs.io/latest/nnls_solving.html), [numeric diff2.2](https://raw.githubusercontent.com/ceres-solver/ceres-solver/2.2.0/include/ceres/internal/numeric_diff.h).
31. [Eigen unsupported LM](https://libeigen.gitlab.io/eigen/docs-nightly/unsupported/classEigen_1_1LevenbergMarquardt.html).
32. [GSL](https://www.gnu.org/software/gsl/), [NLS](https://www.gnu.org/software/gsl/doc/html/nls.html), [interpolation](https://www.gnu.org/software/gsl/doc/html/interp.html).
33. [Armadillo versions](https://arma.sourceforge.net/download.html), [license](https://arma.sourceforge.net/faq.html), [API](https://arma.sourceforge.net/docs.html), [xtensor](https://github.com/xtensor-stack/xtensor), [tags](https://api.github.com/repos/xtensor-stack/xtensor/tags), [FFTW wrapper](https://github.com/xtensor-stack/xtensor-fftw).
34. Gonum [versions](https://pkg.go.dev/gonum.org/v1/gonum?tab=versions), [fourier](https://pkg.go.dev/gonum.org/v1/gonum/dsp/fourier), [mat](https://pkg.go.dev/gonum.org/v1/gonum/mat), [optimize](https://pkg.go.dev/gonum.org/v1/gonum/optimize), [interp](https://pkg.go.dev/gonum.org/v1/gonum/interp), [cubic source.17](https://raw.githubusercontent.com/gonum/gonum/v0.17.0/interp/cubic.go), [stat](https://pkg.go.dev/gonum.org/v1/gonum/stat), [windows](https://pkg.go.dev/gonum.org/v1/gonum/dsp/window).
35. [go-dsp module](https://pkg.go.dev/github.com/mjibson/go-dsp), [go-audio archive](https://github.com/go-audio/audio), [go-audio package](https://pkg.go.dev/github.com/go-audio/audio).
36. [zaf/resample](https://pkg.go.dev/github.com/zaf/resample), [source](https://github.com/zaf/resample).
37. [scientificgo FFT](https://pkg.go.dev/scientificgo.org/fft?tab=versions), [special](https://pkg.go.dev/scientificgo.org/special?tab=versions).
38. [MathNet versions](https://www.nuget.org/packages/MathNet.Numerics/), [managed FFT](https://raw.githubusercontent.com/mathnet/mathnet-numerics/master/src/Numerics/Providers/FourierTransform/ManagedFourierTransformProvider.cs), [interpolation](https://numerics.mathdotnet.com/api/MathNet.Numerics/Interpolate.htm), [Fit](https://numerics.mathdotnet.com/api/MathNet.Numerics/Fit.htm), [special functions](https://numerics.mathdotnet.com/api/MathNet.Numerics/SpecialFunctions.htm).
39. [NWaves versions](https://www.nuget.org/packages/NWaves/), [filters](https://github.com/ar1st0crat/NWaves/wiki/Filters), [operations](https://github.com/ar1st0crat/NWaves/wiki/Operations), [transforms](https://github.com/ar1st0crat/NWaves/wiki/Transforms), [FIR designer](https://raw.githubusercontent.com/ar1st0crat/NWaves/master/NWaves/Filters/Fda/DesignFirFilter.cs), [SG](https://raw.githubusercontent.com/ar1st0crat/NWaves/master/NWaves/Filters/SavitzkyGolayFilter.cs), [resampler](https://raw.githubusercontent.com/ar1st0crat/NWaves/master/NWaves/Operations/Resampler.cs), [float PeakFilter](https://raw.githubusercontent.com/ar1st0crat/NWaves/master/NWaves/Filters/BiQuad/PeakFilter.cs).
40. MathNet [bounded LM API](https://numerics.mathdotnet.com/api/MathNet.Numerics.Optimization/LevenbergMarquardtMinimizer.htm), [v5 LM source](https://raw.githubusercontent.com/mathnet/mathnet-numerics/v5.0.0/src/Numerics/Optimization/LevenbergMarquardtMinimizer.cs), [bound transform](https://raw.githubusercontent.com/mathnet/mathnet-numerics/master/src/Numerics/Optimization/NonlinearMinimizerBase.cs), [v5 numerical Jacobian](https://raw.githubusercontent.com/mathnet/mathnet-numerics/v5.0.0/src/Numerics/Optimization/ObjectiveFunctions/NonlinearObjectiveFunction.cs).
41. [FftSharp](https://www.nuget.org/packages/FftSharp).
42. [Accord archive](https://github.com/accord-net/framework), [Accord3.8](https://www.nuget.org/packages/Accord/3.8.0).
43. [JTransforms](https://github.com/wendykierp/JTransforms), [double arbitrary-length source](https://raw.githubusercontent.com/wendykierp/JTransforms/master/src/main/java/org/jtransforms/fft/DoubleFFT_1D.java).
44. [KMath](https://github.com/SciProgCentre/kmath), [Multik](https://github.com/Kotlin/multik).
45. [Commons Math least squares](https://commons.apache.org/proper/commons-math/userguide/leastsquares.html).
46. [fftea](https://pub.dev/packages/fftea), [SciDart](https://pub.dev/packages/scidart), [SciDart API](https://pub.dev/documentation/scidart/latest/scidart/), [peaks source](https://pub.dev/documentation/scidart/latest/scidart/findPeaks.html).
47. [W3C RBJ Audio EQ Cookbook](https://www.w3.org/TR/audio-eq-cookbook/).
