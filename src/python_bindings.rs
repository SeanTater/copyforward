use crate::tokenization::get_tokenizer;
use crate::{
    Approximate, ApproximateTokens, Config, CopyForward, CopyForwardTokens, Exact, ExactTokens,
    Segment, TokenSegment, approximate, approximate_tokens, exact, exact_tokens,
};
use numpy::{PyArray1, PyReadonlyArray1};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyList, PySequence};

#[pyclass]
#[derive(Debug, Clone)]
struct PyLiteralSegment {
    #[pyo3(get)]
    text: String,
}

#[pymethods]
impl PyLiteralSegment {
    #[new]
    fn new(text: String) -> Self {
        Self { text }
    }

    fn __repr__(&self) -> String {
        format!("LiteralSegment(text='{}')", self.text)
    }
}

#[pyclass]
#[derive(Debug, Clone)]
struct PyReferenceSegment {
    #[pyo3(get)]
    message: usize,
    #[pyo3(get)]
    start: usize,
    #[pyo3(get)]
    len: usize,
}

#[pymethods]
impl PyReferenceSegment {
    #[new]
    fn new(message: usize, start: usize, len: usize) -> Self {
        Self {
            message,
            start,
            len,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "ReferenceSegment(message={}, start={}, len={})",
            self.message, self.start, self.len
        )
    }
}

#[pyclass]
#[derive(Debug, Clone)]
struct PyLiteralTokens {
    #[pyo3(get)]
    tokens: Vec<u32>,
}

#[pymethods]
impl PyLiteralTokens {
    #[new]
    fn new(tokens: Vec<u32>) -> Self {
        Self { tokens }
    }

    fn __repr__(&self) -> String {
        format!("LiteralTokens(tokens={:?})", self.tokens)
    }

    fn as_numpy(&self) -> PyResult<PyObject> {
        Python::with_gil(|py| {
            let arr = PyArray1::<u32>::from_slice(py, &self.tokens);
            Ok(arr.into_py(py))
        })
    }
}

#[pyclass]
#[derive(Debug, Clone)]
struct PyReferenceTokens {
    #[pyo3(get)]
    message: usize,
    #[pyo3(get)]
    start: usize,
    #[pyo3(get)]
    len: usize,
}

#[pymethods]
impl PyReferenceTokens {
    #[new]
    fn new(message: usize, start: usize, len: usize) -> Self {
        Self {
            message,
            start,
            len,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "ReferenceTokens(message={}, start={}, len={})",
            self.message, self.start, self.len
        )
    }
}

#[derive(Debug, Clone)]
enum TextAlg {
    Exact(Exact),
    Approx(Approximate),
}

#[derive(Debug, Clone)]
enum TokensAlg {
    Exact(ExactTokens),
    Approx(ApproximateTokens),
}

#[pyclass(name = "CopyForwardText")]
struct PyCopyForwardText {
    inner: TextAlg,
}

#[pymethods]
impl PyCopyForwardText {
    #[classmethod]
    #[pyo3(signature = (messages, *, exact_mode=true, min_match_len=4, lookback=None, cap_len=64, ncap=64))]
    fn from_texts(
        _cls: &pyo3::types::PyType,
        messages: Vec<Option<String>>,
        exact_mode: bool,
        min_match_len: usize,
        lookback: Option<usize>,
        cap_len: usize,
        ncap: usize,
    ) -> PyResult<Self> {
        let config = Config {
            min_match_len,
            lookback,
            cap_len,
            ncap,
        };
        let inner = if exact_mode {
            TextAlg::Exact(exact(&messages, config))
        } else {
            TextAlg::Approx(approximate(&messages, config))
        };
        Ok(PyCopyForwardText { inner })
    }

    fn segments(&self) -> PyResult<Vec<Vec<PyObject>>> {
        Python::with_gil(|py| {
            let segs = match &self.inner {
                TextAlg::Exact(inner) => CopyForward::segments(inner),
                TextAlg::Approx(inner) => CopyForward::segments(inner),
            };
            Ok(segs
                .into_iter()
                .map(|v| {
                    v.into_iter()
                        .map(|seg| match seg {
                            Segment::Literal(s) => PyLiteralSegment::new(s).into_py(py),
                            Segment::Reference {
                                message_idx,
                                start,
                                len,
                            } => PyReferenceSegment::new(message_idx, start, len).into_py(py),
                        })
                        .collect()
                })
                .collect())
        })
    }

    fn render(&self, replacement: &str) -> Vec<Option<String>> {
        let result = match &self.inner {
            TextAlg::Exact(inner) => CopyForward::render_with_static(inner, replacement),
            TextAlg::Approx(inner) => CopyForward::render_with_static(inner, replacement),
        };
        // Convert empty strings (from None entries) back to None for Python
        result.into_iter().map(|s| if s.is_empty() { None } else { Some(s) }).collect()
    }

    fn compression_ratio(&self) -> f64 {
        let segs = match &self.inner {
            TextAlg::Exact(inner) => CopyForward::segments(inner),
            TextAlg::Approx(inner) => CopyForward::segments(inner),
        };
        let original: usize = match &self.inner {
            TextAlg::Exact(inner) => {
                CopyForward::render_with(inner, |_, _, _, text| text.to_string())
            }
            TextAlg::Approx(inner) => {
                CopyForward::render_with(inner, |_, _, _, text| text.to_string())
            }
        }
        .iter()
        .map(|s| s.len())
        .sum();
        let compressed: usize = segs
            .iter()
            .map(|v| {
                v.iter()
                    .map(|seg| match seg {
                        Segment::Literal(s) => s.len(),
                        Segment::Reference { .. } => 1,
                    })
                    .sum::<usize>()
            })
            .sum();
        if original == 0 {
            1.0
        } else {
            compressed as f64 / original as f64
        }
    }
}

#[pyclass(name = "CopyForwardTokens")]
struct PyCopyForwardTokens {
    inner: TokensAlg,
    tokenizer: Option<Box<dyn crate::tokenization::Tokenize + Send>>,
}

#[pymethods]
impl PyCopyForwardTokens {
    #[classmethod]
    #[pyo3(signature = (messages, *, exact_mode=true, min_match_len=4, lookback=None, cap_len=64, ncap=64))]
    fn from_tokens(
        _cls: &pyo3::types::PyType,
        messages: Vec<Option<Vec<u32>>>,
        exact_mode: bool,
        min_match_len: usize,
        lookback: Option<usize>,
        cap_len: usize,
        ncap: usize,
    ) -> PyResult<Self> {
        let config = Config {
            min_match_len,
            lookback,
            cap_len,
            ncap,
        };
        let inner = if exact_mode {
            TokensAlg::Exact(exact_tokens(&messages, config))
        } else {
            TokensAlg::Approx(approximate_tokens(&messages, config))
        };
        Ok(PyCopyForwardTokens {
            inner,
            tokenizer: None,
        })
    }

    /// Tokenizer opt-in: accept texts and a tokenizer name, return token-mode compressor.
    #[classmethod]
    #[pyo3(signature = (messages, tokenizer, *, exact_mode=true, min_match_len=4, lookback=None, cap_len=64, ncap=64))]
    fn from_texts_with_tokenizer(
        _cls: &pyo3::types::PyType,
        messages: Vec<Option<String>>,
        tokenizer: String,
        exact_mode: bool,
        min_match_len: usize,
        lookback: Option<usize>,
        cap_len: usize,
        ncap: usize,
    ) -> PyResult<Self> {
        let config = Config {
            min_match_len,
            lookback,
            cap_len,
            ncap,
        };
        let mut tok = get_tokenizer(&tokenizer).map_err(PyTypeError::new_err)?;
        let toks: Vec<Option<Vec<u32>>> = messages.into_iter().map(|opt_s| opt_s.map(|s| tok.encode(&s))).collect();
        let inner = if exact_mode {
            TokensAlg::Exact(exact_tokens(&toks, config))
        } else {
            TokensAlg::Approx(approximate_tokens(&toks, config))
        };
        Ok(PyCopyForwardTokens {
            inner,
            tokenizer: Some(tok),
        })
    }

    fn segments(&self) -> PyResult<Vec<Vec<PyObject>>> {
        Python::with_gil(|py| {
            let segs = match &self.inner {
                TokensAlg::Exact(inner) => CopyForwardTokens::segments(inner),
                TokensAlg::Approx(inner) => CopyForwardTokens::segments(inner),
            };
            Ok(segs
                .into_iter()
                .map(|v| {
                    v.into_iter()
                        .map(|seg| match seg {
                            TokenSegment::Literal(toks) => PyLiteralTokens::new(toks).into_py(py),
                            TokenSegment::Reference {
                                message_idx,
                                start,
                                len,
                            } => PyReferenceTokens::new(message_idx, start, len).into_py(py),
                        })
                        .collect()
                })
                .collect())
        })
    }

    #[pyo3(signature = (replacement, *, as_numpy=false))]
    fn render(&self, replacement: &PyAny, as_numpy: bool) -> PyResult<pyo3::PyObject> {
        Python::with_gil(|py| {
            let repl_vec: Vec<u32> = if let Ok(arr) = replacement.extract::<PyReadonlyArray1<u32>>()
            {
                arr.as_slice()?.to_vec()
            } else if let Ok(seq) = replacement.downcast::<PySequence>() {
                let n = seq.len()?;
                let mut v = Vec::with_capacity(n as usize);
                for i in 0..n {
                    let it = seq.get_item(i)?;
                    let val: u64 = it.extract()?;
                    if val > u32::MAX as u64 {
                        return Err(PyTypeError::new_err(
                            "replacement token exceeds u32 range",
                        ));
                    }
                    v.push(val as u32);
                }
                v
            } else {
                return Err(PyTypeError::new_err(
                    "replacement must be a sequence of ints or np.uint32 array",
                ));
            };
            
            let out_vecs: Vec<Vec<u32>> = match &self.inner {
                TokensAlg::Exact(inner) => {
                    CopyForwardTokens::render_with(inner, |_, _, _, _| repl_vec.clone())
                }
                TokensAlg::Approx(inner) => {
                    CopyForwardTokens::render_with(inner, |_, _, _, _| repl_vec.clone())
                }
            };
            if as_numpy {
                let list = PyList::empty(py);
                for v in out_vecs {
                    let arr = PyArray1::<u32>::from_vec(py, v);
                    list.append(arr)?;
                }
                Ok(list.into_py(py))
            } else {
                Ok(out_vecs.to_object(py))
            }
        })
    }

    /// Decode rendered token outputs back to text using the stored tokenizer.
    /// Raises a TypeError if the instance was not created with a tokenizer.
    fn render_texts(&mut self, replacement: &str) -> PyResult<Vec<String>> {
        let tok = self.tokenizer.as_mut().ok_or_else(|| {
            PyTypeError::new_err(
                "render_texts() requires instance created via from_texts_with_tokenizer",
            )
        })?;
        let repl_tokens = tok.encode(replacement);
        let tokens: Vec<Vec<u32>> = match &self.inner {
            TokensAlg::Exact(inner) => {
                CopyForwardTokens::render_with(inner, |_, _, _, _| repl_tokens.clone())
            }
            TokensAlg::Approx(inner) => {
                CopyForwardTokens::render_with(inner, |_, _, _, _| repl_tokens.clone())
            }
        };
        Ok(tokens.into_iter().map(|v| tok.decode(&v)).collect())
    }
}

#[pymodule]
fn copyforward(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyCopyForwardText>()?;
    m.add_class::<PyCopyForwardTokens>()?;
    m.add_class::<PyLiteralSegment>()?;
    m.add_class::<PyReferenceSegment>()?;
    m.add_class::<PyLiteralTokens>()?;
    m.add_class::<PyReferenceTokens>()?;
    Ok(())
}
