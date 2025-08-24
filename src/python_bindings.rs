#![allow(unsafe_op_in_unsafe_fn)]
use pyo3::prelude::*;
use crate::{LongestMatch, CopyForward};

#[pyclass]
struct PyLongestMatch {
    inner: LongestMatch,
}

#[allow(unsafe_op_in_unsafe_fn)]
#[pymethods]
impl PyLongestMatch {
    #[new]
    fn new(messages: Vec<String>, min_match_len: Option<usize>, lookback: Option<usize>) -> Self {
        let refs: Vec<&str> = messages.iter().map(|s| s.as_str()).collect();
        let cfg = crate::LongestMatchConfig { min_match_len: min_match_len.unwrap_or(1), lookback };
        PyLongestMatch { inner: LongestMatch::with_config(&cfg, &refs) }
    }

    fn segments(&self) -> Vec<Vec<String>> {
        <LongestMatch as CopyForward>::segments(&self.inner)
            .into_iter()
            .map(|v| {
                v.into_iter()
                    .map(|seg| match seg {
                        crate::Segment::Literal(s) => format!("L:{s}"),
                        crate::Segment::Reference { message_idx, start, len } => format!("R:{message_idx}:{start}+{len}"),
                    })
                    .collect()
            })
            .collect()
    }

    fn render_with_static(&self, replacement: &str) -> Vec<String> {
        <LongestMatch as CopyForward>::render_with(&self.inner, |_m, _s, _l, _text| replacement.to_string())
    }
}

#[pymodule]
fn copyforward(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyLongestMatch>()?;
    Ok(())
}


