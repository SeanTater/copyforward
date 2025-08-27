use std::collections::HashMap;

/// Minimal tokenizer trait for Python auto-tokenization.
/// Implementations should provide deterministic encode/decode within an instance.
pub trait Tokenize {
    fn encode(&mut self, text: &str) -> Vec<u32>;
    fn decode(&self, ids: &[u32]) -> String;
}

/// A very simple whitespace tokenizer with an internal word-id map.
/// - Splits on Unicode whitespace.
/// - Assigns incremental u32 IDs to new words as encountered.
/// - Decodes by joining tokens with a single space.
pub struct WhitespaceTokenizer {
    vocab: HashMap<String, u32>,
    rev: Vec<String>,
}

impl WhitespaceTokenizer {
    pub fn new() -> Self {
        Self {
            vocab: HashMap::new(),
            rev: Vec::new(),
        }
    }
    fn id_for(&mut self, token: &str) -> u32 {
        if let Some(&id) = self.vocab.get(token) {
            return id;
        }
        let id = self.rev.len() as u32;
        self.vocab.insert(token.to_string(), id);
        self.rev.push(token.to_string());
        id
    }
}

impl Default for WhitespaceTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tokenize for WhitespaceTokenizer {
    fn encode(&mut self, text: &str) -> Vec<u32> {
        text.split_whitespace().map(|t| self.id_for(t)).collect()
    }
    fn decode(&self, ids: &[u32]) -> String {
        let mut out = String::new();
        for (i, &id) in ids.iter().enumerate() {
            if (id as usize) < self.rev.len() {
                if i > 0 {
                    out.push(' ');
                }
                out.push_str(&self.rev[id as usize]);
            } else {
                // Unknown id; insert placeholder
                if i > 0 {
                    out.push(' ');
                }
                out.push_str("<UNK>");
            }
        }
        out
    }
}

/// Get a tokenizer by name.
/// Currently supports:
/// - "whitespace": simple whitespace tokenizer.
///
/// Future: "hf:<model>" via the `tokenizers` feature.
pub fn get_tokenizer(name: &str) -> Result<Box<dyn Tokenize + Send>, String> {
    match name {
        "whitespace" => Ok(Box::new(WhitespaceTokenizer::new())),
        _ if name.starts_with("hf:") => {
            let model = &name[3..];
            get_hf_tokenizer(model)
        }
        _ if name.starts_with("file:") => {
            let path = &name[5..];
            get_file_tokenizer(path)
        }
        _ => Err(format!(
            "unknown tokenizer '{}'. Available: 'whitespace', 'hf:<model>' (with feature), 'file:<path>' (with feature)",
            name
        )),
    }
}

#[cfg(feature = "tokenizers")]
mod hf_impl {
    use super::Tokenize;
    use std::path::Path;
    use tokenizers::Tokenizer;

    pub struct HfTokenizer {
        inner: Tokenizer,
    }

    impl HfTokenizer {
        pub fn from_file(path: &str) -> Result<Self, String> {
            let inner = Tokenizer::from_file(Path::new(path))
                .map_err(|e| format!("failed to load tokenizer from file: {}", e))?;
            Ok(Self { inner })
        }

        #[cfg(feature = "hf-hub")]
        pub fn from_pretrained(_name: &str) -> Result<Self, String> {
            Err(
                "hf-hub loading not implemented for tokenizers v0.15; load from file instead"
                    .to_string(),
            )
        }
    }

    impl Tokenize for HfTokenizer {
        fn encode(&mut self, text: &str) -> Vec<u32> {
            // Truncation=false, padding=false by default
            let enc = self.inner.encode(text, true).expect("encoding failed");
            enc.get_ids().to_vec()
        }
        fn decode(&self, ids: &[u32]) -> String {
            self.inner.decode(ids, true).unwrap_or_default()
        }
    }

    pub fn get_hf_tokenizer(name: &str) -> Result<Box<dyn Tokenize + Send>, String> {
        #[cfg(feature = "hf-hub")]
        {
            Ok(Box::new(HfTokenizer::from_pretrained(name)?))
        }
        #[cfg(not(feature = "hf-hub"))]
        {
            Err("hf:<model> requires building with the 'hf-hub' feature".to_string())
        }
    }

    pub fn get_file_tokenizer(path: &str) -> Result<Box<dyn Tokenize + Send>, String> {
        Ok(Box::new(HfTokenizer::from_file(path)?))
    }
}

#[cfg(not(feature = "tokenizers"))]
fn get_hf_tokenizer(_name: &str) -> Result<Box<dyn Tokenize + Send>, String> {
    Err("HuggingFace tokenizers requires building with the 'tokenizers' feature".to_string())
}

#[cfg(not(feature = "tokenizers"))]
fn get_file_tokenizer(_path: &str) -> Result<Box<dyn Tokenize + Send>, String> {
    Err("Loading tokenizers from file requires building with the 'tokenizers' feature".to_string())
}

#[cfg(feature = "tokenizers")]
use hf_impl::{get_file_tokenizer, get_hf_tokenizer};
