#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! PyO3 module `impulcifer_native` (built with maturin, feature `python`).
//! Boundary policy: copy inputs (`PyReadonlyArray` -> Vec), detach for long
//! work, return newly owned arrays. Implemented by an ASTRA packet.

#[cfg(feature = "python")]
mod module {
    use pyo3::prelude::*;

    #[pyfunction]
    fn version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    #[pymodule]
    fn impulcifer_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add_function(wrap_pyfunction!(version, m)?)?;
        Ok(())
    }
}
