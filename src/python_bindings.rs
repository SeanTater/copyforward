use pyo3::prelude::*;
use crate::LongestMatch;

#[pyclass]
struct PyLongestMatch {
    inner: LongestMatch,
}

#[pymethods]
impl PyLongestMatch {
    #[new]
    fn new(messages: Vec<String>) -> Self {
        let refs: Vec<&str> = messages.iter().map(|s| s.as_str()).collect();
        PyLongestMatch { inner: LongestMatch::from_messages(&refs) }
    }

    fn segments(&self) -> Vec<Vec<String>> {
        self.inner.segments().into_iter().map(|v| {
            v.into_iter().map(|seg| match seg {
                crate::Segment::Literal(s) => format!("L:{}", s),
                crate::Segment::Reference { message_idx, start, len } => format!("R:{}:{}+{}", message_idx, start, len),
            }).collect()
        }).collect()
    }

    fn render_with_static(&self, replacement: &str) -> Vec<String> {
        self.inner.render_with(|_m, _s, _l, _text| replacement.to_string())
    }
}

#[pymodule]
fn copyforward_py(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyLongestMatch>()?;
    Ok(())
}


