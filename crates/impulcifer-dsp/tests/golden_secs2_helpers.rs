#![forbid(unsafe_code)]
#![allow(dead_code)]
use impulcifer_dsp::fft::Complex64;
use serde_json::Value;
use std::{fs, path::PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/migration/goldens")
}
pub fn fixture(name: &str) -> Value {
    serde_json::from_slice(&fs::read(root().join(format!("s2_{name}.json"))).unwrap()).unwrap()
}
pub fn fixtures(prefix: &str) -> Vec<(String, Value)> {
    let mut names: Vec<_> = fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.starts_with(&format!("s2_{prefix}")) && n.ends_with(".json"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "missing family {prefix}");
    names
        .into_iter()
        .map(|n| {
            let name = n
                .trim_start_matches("s2_")
                .trim_end_matches(".json")
                .to_owned();
            let v = fixture(&name);
            (name, v)
        })
        .collect()
}
/// Whether Rust's f64 math here is the oracle's C library bit for bit
/// (glibc on Linux; `secs2_libm_probe.json`). The oracle's fit paths and
/// last-bit equalities only hold there: another libm moves the residual by an
/// ulp, and the trust-region path leaves the oracle at once, exactly as the
/// oracle's own AVX-512 variant does. Elsewhere the exact gates widen to
/// `PLATFORM_RTOL` and the chaotic ones (fit traces) report without failing;
/// the end-to-end comparisons against SECS's spread across computers hold on
/// every platform, the RC band caps with a wider margin.
pub fn oracle_libm() -> bool {
    static SAME: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SAME.get_or_init(|| {
        // Lets the platform paths be exercised on the oracle's own libm.
        if std::env::var_os("SECS2_ASSUME_FOREIGN_LIBM").is_some() {
            println!("PLATFORM libm treated as foreign (SECS2_ASSUME_FOREIGN_LIBM)");
            return false;
        }
        let probe: Value = serde_json::from_slice(
            &fs::read(root().join("secs2_libm_probe.json")).unwrap(),
        )
        .unwrap();
        let cases = &probe["cases"];
        let unary = |name: &str, f: fn(f64) -> f64| {
            cases[name].as_array().unwrap().iter().all(|c| {
                f(c[0].as_f64().unwrap()).to_bits() == c[1].as_f64().unwrap().to_bits()
            })
        };
        let same = unary("ln", f64::ln)
            && unary("log10", f64::log10)
            && unary("exp", f64::exp)
            && unary("sin", f64::sin)
            && unary("cos", f64::cos)
            && cases["pow"].as_array().unwrap().iter().all(|c| {
                c[0].as_f64().unwrap().powf(c[1].as_f64().unwrap()).to_bits()
                    == c[2].as_f64().unwrap().to_bits()
            });
        if !same {
            println!("PLATFORM libm differs from the oracle's: exact gates widen to {PLATFORM_RTOL:e}, fit traces report only");
        }
        same
    })
}
/// Relative budget for stages that are exact on the oracle's libm.
pub const PLATFORM_RTOL: f64 = 1e-9;
/// `rtol` on the oracle's libm, at least `PLATFORM_RTOL` elsewhere.
pub fn rtol(exact: f64) -> f64 {
    if oracle_libm() {
        exact
    } else {
        exact.max(PLATFORM_RTOL)
    }
}
/// Bit equality on the oracle's libm, `PLATFORM_RTOL` elsewhere.
pub fn assert_same(name: &str, got: &[f64], expected: &[f64]) {
    if oracle_libm() {
        assert_eq!(got, expected, "{name}");
        return;
    }
    assert_eq!(got.len(), expected.len(), "{name}: shape");
    for (i, (g, e)) in got.iter().zip(expected).enumerate() {
        assert!(
            (g - e).abs() <= PLATFORM_RTOL * e.abs().max(1.),
            "{name}[{i}]: got={g:e} expected={e:e}"
        );
    }
}
pub fn number(v: &Value) -> f64 {
    match v.as_str() {
        Some("nan") => f64::NAN,
        Some("inf" | "+inf") => f64::INFINITY,
        Some("-inf") => f64::NEG_INFINITY,
        _ => v.as_f64().expect("numeric fixture value"),
    }
}
pub fn integer(v: &Value) -> usize {
    v.as_u64().unwrap().try_into().unwrap()
}
pub fn array(v: &Value) -> Vec<f64> {
    if let Some(file) = v.get("file") {
        let name = file.as_str().unwrap();
        assert!(name.starts_with("s2_") && !name.contains('/') && !name.contains('\\'));
        let path = root().join(name);
        let bytes = fs::read(&path).unwrap();
        assert_eq!(
            bytes.len(),
            integer(&v["length"]) * 8,
            "{name}: byte length"
        );
        assert_eq!(
            sha256(&bytes),
            v["sha256"].as_str().unwrap(),
            "{name}: SHA256"
        );
        return bytes
            .chunks_exact(8)
            .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
            .collect();
    }
    let a = v.as_array().expect("array or sidecar descriptor");
    if a.first().is_some_and(|v| v.get("file").is_some()) {
        return a.iter().flat_map(array).collect();
    }
    a.iter().map(number).collect()
}
pub fn complex(v: &Value) -> Vec<Complex64> {
    if v.get("re").is_some() {
        let re = array(&v["re"]);
        let im = array(&v["im"]);
        assert_eq!(re.len(), im.len(), "complex shape");
        re.into_iter()
            .zip(im)
            .map(|(r, i)| Complex64::new(r, i))
            .collect()
    } else {
        v.as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let a = array(v);
                assert_eq!(a.len(), 2);
                Complex64::new(a[0], a[1])
            })
            .collect()
    }
}
pub fn bands(v: &Value) -> Vec<(f64, f64, f64)> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| {
            let a = array(v);
            assert_eq!(a.len(), 3);
            (a[0], a[1], a[2])
        })
        .collect()
}
#[derive(Default)]
pub struct Errors {
    absolute: f64,
    relative: f64,
    count: usize,
    failures: Vec<String>,
}
impl Errors {
    pub fn compare(&mut self, name: &str, got: &[f64], expected: &[f64], rtol: f64, atol: f64) {
        assert_eq!(got.len(), expected.len(), "{name}: shape");
        let rtol = self::rtol(rtol);
        let (mut se, mut sr) = (0., 0.);
        let mut first = None;
        for (i, (&g, &e)) in got.iter().zip(expected).enumerate() {
            self.count += 1;
            if !e.is_finite() {
                if !((e.is_nan() && g.is_nan()) || e == g) && first.is_none() {
                    first = Some(format!("{name}[{i}]: nonfinite mask got={g} expected={e}"));
                }
                continue;
            }
            let error = (g - e).abs();
            if (!g.is_finite() || error > atol + rtol * e.abs()) && first.is_none() {
                first = Some(format!(
                    "{name}[{i}]: got={g:.17e} expected={e:.17e} error={error:.17e}"
                ));
            }
            self.absolute = self.absolute.max(error);
            se += error * error;
            sr += e * e;
        }
        let rel = if sr > 0. { (se / sr).sqrt() } else { se.sqrt() };
        self.relative = self.relative.max(rel);
        if let Some(f) = first {
            self.failures.push(f);
        }
    }
    pub fn real(&mut self, name: &str, g: &[f64], e: &[f64]) {
        // golden_batch1's absolute floor for near-zero components, plus the
        // README-secs2 1e-12 relative budget (also enforced over the full array).
        self.compare(name, g, e, 1e-12, 1e-12);
        let finite: Vec<_> = g.iter().zip(e).filter(|(_, e)| e.is_finite()).collect();
        let energy: f64 = finite.iter().map(|(_, e)| **e * **e).sum();
        let error: f64 = finite.iter().map(|(g, e)| (**g - **e).powi(2)).sum();
        if energy > 0. && (error / energy).sqrt() > rtol(1e-12) {
            self.failures
                .push(format!("{name}: relative L2 exceeds {:e}", rtol(1e-12)));
        }
    }
    pub fn complex(&mut self, name: &str, g: &[Complex64], e: &[Complex64]) {
        assert_eq!(g.len(), e.len(), "{name}: complex shape");
        self.real(
            name,
            &g.iter().flat_map(|v| [v.re, v.im]).collect::<Vec<_>>(),
            &e.iter().flat_map(|v| [v.re, v.im]).collect::<Vec<_>>(),
        );
    }
    pub fn finish(self, family: &str) {
        println!(
            "MEASURE {family}: max_abs={:.17e}, max_relative_l2={:.17e}, values={}",
            self.absolute, self.relative, self.count
        );
        assert!(self.failures.is_empty(), "{}", self.failures.join("\n"));
    }
}

#[test]
fn sha256_known_answers() {
    assert_eq!(
        sha256(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

// Same dependency-free SHA-256 used by impulcifer-io fixture gates.
fn sha256(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut data = bytes.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&((bytes.len() as u64) * 8).to_be_bytes());
    for chunk in data.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes(word.try_into().unwrap());
        }
        for i in 16..64 {
            let x = w[i - 15];
            let y = w[i - 2];
            let s0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
            let s1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut z] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let t1 = z
                .wrapping_add(s1)
                .wrapping_add((e & f) ^ (!e & g))
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let t2 = s0.wrapping_add((a & b) ^ (a & c) ^ (b & c));
            z = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (value, add) in h.iter_mut().zip([a, b, c, d, e, f, g, z]) {
            *value = value.wrapping_add(add);
        }
    }
    h.iter().map(|n| format!("{n:08x}")).collect()
}
