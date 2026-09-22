use std::path::PathBuf;
use std::sync::Mutex;
use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use tracing::{debug, info, warn};

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

/// Thread-safe ONNX Runtime (`ort`) text embedder for semantic search.
/// Models are lazily loaded on first embedding request.
pub struct CodeEmbedder {
    model: Mutex<Option<TextEmbedding>>,
    model_name: EmbeddingModel,
    cache_dir: Option<PathBuf>,
}

impl Default for CodeEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeEmbedder {
    /// Create a new CodeEmbedder using the default BGE Small English v1.5 model.
    pub fn new() -> Self {
        Self {
            model: Mutex::new(None),
            model_name: EmbeddingModel::BGESmallENV15,
            cache_dir: None,
        }
    }

    /// Create a new CodeEmbedder with a specific model and cache directory.
    pub fn with_options(model_name: EmbeddingModel, cache_dir: Option<PathBuf>) -> Self {
        Self {
            model: Mutex::new(None),
            model_name,
            cache_dir,
        }
    }

    /// Check if the model is currently initialized and ready.
    pub fn is_initialized(&self) -> bool {
        self.model.lock().map(|m| m.is_some()).unwrap_or(false)
    }

    /// Ensure the model is initialized. Downloads and instantiates the ONNX model if needed.
    pub fn ensure_initialized(&self) -> Result<()> {
        let mut guard = self
            .model
            .lock()
            .map_err(|e| anyhow::anyhow!("Embedder mutex poisoned: {}", e))?;

        if guard.is_none() {
            debug!("Initializing ONNX Runtime embedding model: {:?}", self.model_name);
            let mut opts = TextInitOptions::new(self.model_name.clone());
            if let Some(ref cache) = self.cache_dir {
                opts = opts.with_cache_dir(cache.clone());
            }
            opts = opts.with_show_download_progress(false);

            match TextEmbedding::try_new(opts) {
                Ok(instance) => {
                    info!("ONNX Runtime embedding model initialized successfully");
                    *guard = Some(instance);
                }
                Err(err) => {
                    warn!("Failed to initialize ONNX Runtime embedding model: {}", err);
                    return Err(anyhow::anyhow!("ONNX embedding initialization failed: {}", err));
                }
            }
        }
        Ok(())
    }

    /// Generate vector embeddings for a slice of text strings.
    pub fn embed<S: AsRef<str> + Send + Sync>(&self, texts: &[S]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        self.ensure_initialized()?;
        let mut guard = self
            .model
            .lock()
            .map_err(|e| anyhow::anyhow!("Embedder mutex poisoned: {}", e))?;

        let model = guard
            .as_mut()
            .context("Embedding model not loaded")?;

        let text_refs: Vec<&str> = texts.iter().map(|t| t.as_ref()).collect();
        let embeddings = model
            .embed(text_refs, Some(64))
            .map_err(|e| anyhow::anyhow!("ONNX inference error: {}", e))?;

        Ok(embeddings)
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
