use std::collections::HashMap;
use std::path::{Path, PathBuf};
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};

use crate::storage::ManifestEntry;

#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub relative_path: String,
    pub language: String,
    pub content_hash: String,
    pub size: u64,
    pub mtime: f64,
}

pub struct FileScanner {
    root: PathBuf,
    canonical_root: PathBuf,
    ignore_patterns: Vec<String>,
}

impl FileScanner {
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        let root = root.as_ref().to_path_buf();
        // Canonicalize now so the symlink check compares apples-to-apples.
        // On macOS /tmp is a symlink to /private/tmp, so without this the
        // confinement check would always reject files under /tmp.
        let canonical_root = root.canonicalize().unwrap_or_else(|_| root.clone());
        Self {
            root,
            canonical_root,
            ignore_patterns: vec![
                "target".to_string(),
                "node_modules".to_string(),
                ".git".to_string(),
                ".venv".to_string(),
                "venv".to_string(),
                "__pycache__".to_string(),
                ".codeatlas".to_string(),
                "dist".to_string(),
                "build".to_string(),
            ],
        }
    }

    pub fn scan(&self) -> Vec<ScannedFile> {
        self.scan_with_manifest(&HashMap::new(), false)
    }

    pub fn scan_with_manifest(
        &self,
        manifest: &HashMap<String, ManifestEntry>,
        fast_mode: bool,
    ) -> Vec<ScannedFile> {
        let mut builder = WalkBuilder::new(&self.root);
        builder.hidden(true); // ignore hidden files/directories (.git, .codeatlas, etc.)
        builder.parents(true);
        builder.git_ignore(true);
        builder.git_global(true);
        builder.git_exclude(true);

        let mut results = Vec::new();

        for entry in builder.build().flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            // Skip AppleDouble files or OS metadata
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("._") || name.starts_with(".DS_Store") {
                    continue;
                }
            }

            // Check if ignored by custom patterns
            let path_str = path.to_string_lossy();
            if self.ignore_patterns.iter().any(|pat| path_str.contains(pat)) {
                continue;
            }

            if let Some(lang) = detect_language(path) {
                // Symlink confinement: skip any file whose canonical path escapes root
                if let Ok(real) = path.canonicalize() {
                    if !real.starts_with(&self.canonical_root) {
                        continue;
                    }
                }

                let metadata = entry.metadata().ok();
                let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                let mtime = metadata
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0);

                let relative_path = path
                    .strip_prefix(&self.root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();

                // In fast mode, if mtime and size match existing manifest entry, reuse content_hash
                let content_hash = if fast_mode {
                    if let Some(prev) = manifest.get(&relative_path) {
                        if prev.size == size && (prev.mtime - mtime).abs() < 0.001 {
                            Some(prev.content_hash.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                let content_hash = match content_hash {
                    Some(hash) => hash,
                    None => {
                        if let Ok(content) = std::fs::read(path) {
                            let mut hasher = Sha256::new();
                            hasher.update(&content);
                            format!("{:x}", hasher.finalize())
                        } else {
                            continue;
                        }
                    }
                };

                results.push(ScannedFile {
                    path: path.to_path_buf(),
                    relative_path,
                    language: lang.to_string(),
                    content_hash,
                    size,
                    mtime,
                });
            }
        }

        results
    }
}

pub fn detect_language(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "py" => Some("python"),
        "rs" => Some("rust"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "go" => Some("go"),
        "java" => Some("java"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" => Some("cpp"),
        _ => None,
    }
}
