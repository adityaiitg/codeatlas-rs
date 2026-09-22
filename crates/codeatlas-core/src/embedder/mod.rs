use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use anyhow::{anyhow, bail, Context, Result};
use half::f16;
use hf_hub::api::sync::ApiBuilder;
use safetensors::tensor::Dtype;
use safetensors::SafeTensors;
use serde_json::Value;
use tokenizers::Tokenizer;
use tracing::{debug, info};

/// Default Model2Vec static code embedding model.
/// Exact same model used by Python CodeAtlas for 100% representation parity.
pub const DEFAULT_MODEL2VEC_ID: &str = "minishlab/potion-code-16M-v2";

/// Serialize an f32 vector into little-endian bytes for compact BLOB storage in SQLite.
pub fn embedding_to_bytes(vec: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vec.len() * 4);
    for &val in vec {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

/// Deserialize little-endian bytes from a SQLite BLOB back into an f32 vector.
pub fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect()
}

/// Compute cosine similarity between two float vectors.
/// If vectors are L2-normalized, this is equivalent to dot product.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    if norm_a <= 0.0 || norm_b <= 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

/// Static Model2Vec embedding engine implemented in 100% pure Rust.
/// No ONNX Runtime (`ort`), no PyTorch, and zero C++ dynamic library dependencies.
pub struct Model2VecInner {
    tokenizer: Tokenizer,
    embeddings: Vec<f32>,
    dim: usize,
    vocab_size: usize,
    normalize: bool,
}

impl Model2VecInner {
    /// Load a Model2Vec model from local safetensors, tokenizer.json, and optional config.json.
    pub fn from_files(
        model_path: &Path,
        tokenizer_path: &Path,
        config_path: Option<&Path>,
    ) -> Result<Self> {
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow!("Failed to load tokenizer from {}: {}", tokenizer_path.display(), e))?;

        let normalize = if let Some(cfg_p) = config_path {
            if let Ok(file) = File::open(cfg_p) {
                let v: Value = serde_json::from_reader(file).unwrap_or(Value::Null);
                v.get("normalize").and_then(Value::as_bool).unwrap_or(true)
            } else {
                true
            }
        } else {
            true
        };

        let model_bytes = std::fs::read(model_path)
            .with_context(|| format!("Failed to read safetensors at {}", model_path.display()))?;

        let tensors = SafeTensors::deserialize(&model_bytes)
            .context("Failed to parse safetensors data")?;

        let tensor = tensors
            .tensor("embeddings")
            .or_else(|_| tensors.tensor("0"))
            .context("No 'embeddings' tensor found in safetensors")?;

        let shape = tensor.shape();
        if shape.len() != 2 {
            bail!("Expected 2D embedding tensor, got shape {:?}", shape);
        }
        let vocab_size = shape[0];
        let dim = shape[1];
        let raw = tensor.data();

        let embeddings: Vec<f32> = match tensor.dtype() {
            Dtype::F32 => raw
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect(),
            Dtype::F16 => raw
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| f16::from_le_bytes(*b).to_f32())
                .collect(),
            Dtype::I8 => raw.iter().map(|&b| b as i8 as f32).collect(),
            other => bail!("Unsupported tensor dtype: {:?}", other),
        };

        if embeddings.len() != vocab_size * dim {
            bail!(
                "Mismatch in embedding matrix length: expected {} ({}x{}), got {}",
                vocab_size * dim,
                vocab_size,
                dim,
                embeddings.len()
            );
        }

        Ok(Self {
            tokenizer,
            embeddings,
            dim,
            vocab_size,
            normalize,
        })
    }

    /// Encode a single text string into an L2-normalized dense vector.
    pub fn encode_one(&self, text: &str) -> Result<Vec<f32>> {
        let encoding = self
            .tokenizer
            .encode(text, false)
            .map_err(|e| anyhow!("Tokenization failed: {}", e))?;

        let ids = encoding.get_ids();
        let mut out = vec![0.0f32; self.dim];

        if ids.is_empty() {
            return Ok(out);
        }

        let mut count = 0usize;
        for &id in ids {
            let id_idx = id as usize;
            if id_idx < self.vocab_size {
                let offset = id_idx * self.dim;
                let slice = &self.embeddings[offset..offset + self.dim];
                for i in 0..self.dim {
                    out[i] += slice[i];
                }
                count += 1;
            }
        }

        if count == 0 {
            return Ok(out);
        }

        if self.normalize {
            let norm = out.iter().map(|&v| v * v).sum::<f32>().sqrt().max(1e-12);
            for v in &mut out {
                *v /= norm;
            }
        } else {
            let c = count as f32;
            for v in &mut out {
                *v /= c;
            }
        }

        Ok(out)
    }

    /// Encode a batch of texts into dense vectors.
    pub fn encode_batch<S: AsRef<str> + Sync>(&self, texts: &[S]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.encode_one(t.as_ref())).collect()
    }

    pub fn dim(&self) -> usize {
        self.dim
    }
}

/// Thread-safe Model2Vec embedder for semantic code search.
/// Models are lazily loaded on first embedding request.
pub struct CodeEmbedder {
    model: RwLock<Option<Arc<Model2VecInner>>>,
    model_id: String,
    local_dir: Option<PathBuf>,
}

impl Default for CodeEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeEmbedder {
    /// Create a new CodeEmbedder with the default `minishlab/potion-code-16M-v2` model.
    pub fn new() -> Self {
        Self {
            model: RwLock::new(None),
            model_id: DEFAULT_MODEL2VEC_ID.to_string(),
            local_dir: None,
        }
    }

    /// Create a new CodeEmbedder with a specific HuggingFace model repo ID.
    pub fn with_model(model_id: impl Into<String>) -> Self {
        Self {
            model: RwLock::new(None),
            model_id: model_id.into(),
            local_dir: None,
        }
    }

    /// Create a CodeEmbedder that loads from a local directory containing model files.
    pub fn with_local_dir<P: Into<PathBuf>>(dir: P) -> Self {
        Self {
            model: RwLock::new(None),
            model_id: DEFAULT_MODEL2VEC_ID.to_string(),
            local_dir: Some(dir.into()),
        }
    }

    /// Check if the model is currently initialized and loaded in memory.
    pub fn is_initialized(&self) -> bool {
        self.model.read().map(|m| m.is_some()).unwrap_or(false)
    }

    /// Ensure the Model2Vec model is loaded. Downloads via HuggingFace Hub on first use.
    pub fn ensure_initialized(&self) -> Result<()> {
        {
            let guard = self
                .model
                .read()
                .map_err(|e| anyhow!("Embedder lock poisoned: {}", e))?;
            if guard.is_some() {
                return Ok(());
            }
        }

        let mut guard = self
            .model
            .write()
            .map_err(|e| anyhow!("Embedder lock poisoned: {}", e))?;

        if guard.is_none() {
            debug!("Initializing pure-Rust Model2Vec embedder: {}", self.model_id);

            let inner = if let Some(ref dir) = self.local_dir {
                let model_path = dir.join("model.safetensors");
                let tok_path = dir.join("tokenizer.json");
                let cfg_path = dir.join("config.json");
                Model2VecInner::from_files(&model_path, &tok_path, Some(&cfg_path))?
            } else {
                let api = ApiBuilder::new()
                    .build()
                    .map_err(|e| anyhow!("Failed to build HuggingFace API: {}", e))?;
                let repo = api.model(self.model_id.clone());

                let model_path = repo
                    .get("model.safetensors")
                    .map_err(|e| anyhow!("Failed to download model.safetensors for {}: {}", self.model_id, e))?;
                let tokenizer_path = repo
                    .get("tokenizer.json")
                    .map_err(|e| anyhow!("Failed to download tokenizer.json for {}: {}", self.model_id, e))?;
                let config_path = repo.get("config.json").ok();

                Model2VecInner::from_files(&model_path, &tokenizer_path, config_path.as_deref())?
            };

            info!(
                "Model2Vec embedding model '{}' loaded (dim: {}, vocab: {})",
                self.model_id,
                inner.dim(),
                inner.vocab_size
            );
            *guard = Some(Arc::new(inner));
        }

        Ok(())
    }

    /// Generate vector embeddings for a slice of text strings using Model2Vec.
    pub fn embed<S: AsRef<str> + Sync>(&self, texts: &[S]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        self.ensure_initialized()?;
        let guard = self
            .model
            .read()
            .map_err(|e| anyhow!("Embedder lock poisoned: {}", e))?;

        let model = guard
            .as_ref()
            .context("Embedding model not loaded")?;

        model.encode_batch(texts)
    }

    /// Embed a single search query text.
    pub fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        let results = self.embed(&[query])?;
        results
            .into_iter()
            .next()
            .context("No embedding returned for query")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_serialization_roundtrip() {
        let original = vec![0.1234f32, -0.5678f32, 1.0f32, 0.0f32, -1.0f32];
        let bytes = embedding_to_bytes(&original);
        assert_eq!(bytes.len(), original.len() * 4);
        let restored = bytes_to_embedding(&bytes);
        assert_eq!(original, restored);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0f32, 0.0f32, 0.0f32];
        let b = vec![1.0f32, 0.0f32, 0.0f32];
        let c = vec![0.0f32, 1.0f32, 0.0f32];
        let d = vec![-1.0f32, 0.0f32, 0.0f32];

        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-5);
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 1e-5);
        assert!((cosine_similarity(&a, &d) - (-1.0)).abs() < 1e-5);
    }
}
