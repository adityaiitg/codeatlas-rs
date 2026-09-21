use std::path::{Path, PathBuf};
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};

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
    ignore_patterns: Vec<String>,
}

impl FileScanner {
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
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
                if let Ok(content) = std::fs::read(path) {
                    let mut hasher = Sha256::new();
                    hasher.update(&content);
                    let content_hash = format!("{:x}", hasher.finalize());

                    let metadata = entry.metadata().ok();
                    let size = metadata.as_ref().map(|m| m.len()).unwrap_or(content.len() as u64);
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
