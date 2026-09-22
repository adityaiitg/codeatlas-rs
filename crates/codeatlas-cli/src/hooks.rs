use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

const HOOK_START: &str = "# >>> CodeAtlas Hook >>>";
const HOOK_END: &str = "# <<< CodeAtlas Hook <<<";
const HOOK_BODY: &str = r#"
# >>> CodeAtlas Hook >>>
if command -v codeatlas >/dev/null 2>&1; then
    (codeatlas index --fast >/dev/null 2>&1 &)
fi
# <<< CodeAtlas Hook <<<
"#;

fn find_git_hooks_dir(root: &Path) -> Option<PathBuf> {
    let mut curr = root.to_path_buf();
    loop {
        let git_dir = curr.join(".git");
        if git_dir.is_dir() {
            return Some(git_dir.join("hooks"));
        }
        if !curr.pop() {
            break;
        }
    }
    None
}

pub fn install_git_hooks(root: &Path) -> Result<Vec<String>> {
    let hooks_dir = find_git_hooks_dir(root)
        .context("Could not find a .git repository directory in current or parent paths")?;

    fs::create_dir_all(&hooks_dir)
        .with_context(|| format!("Creating directory {}", hooks_dir.display()))?;

    let hook_names = ["post-commit", "post-checkout", "post-merge"];
    let mut installed = Vec::new();

    for name in hook_names {
        let file_path = hooks_dir.join(name);
        if file_path.exists() {
            let existing = fs::read_to_string(&file_path)
                .with_context(|| format!("Reading {}", file_path.display()))?;
            if existing.contains(HOOK_START) {
                installed.push(format!("{} (already present)", name));
                continue;
            }
            let new_content = format!("{}\n{}", existing.trim_end(), HOOK_BODY);
            fs::write(&file_path, new_content)
                .with_context(|| format!("Writing {}", file_path.display()))?;
        } else {
            let new_content = format!("#!/bin/sh\n{}", HOOK_BODY);
            fs::write(&file_path, new_content)
                .with_context(|| format!("Writing {}", file_path.display()))?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&file_path, fs::Permissions::from_mode(0o755));
        }

        installed.push(name.to_string());
    }

    Ok(installed)
}

pub fn uninstall_git_hooks(root: &Path) -> Result<Vec<String>> {
    let hooks_dir = match find_git_hooks_dir(root) {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    let hook_names = ["post-commit", "post-checkout", "post-merge"];
    let mut uninstalled = Vec::new();

    for name in hook_names {
        let file_path = hooks_dir.join(name);
        if !file_path.exists() {
            continue;
        }

        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("Reading {}", file_path.display()))?;
        if !content.contains(HOOK_START) {
            continue;
        }

        let lines: Vec<&str> = content.lines().collect();
        let mut filtered = Vec::new();
        let mut skipping = false;

        for line in lines {
            if line.contains(HOOK_START) {
                skipping = true;
                continue;
            }
            if line.contains(HOOK_END) {
                skipping = false;
                continue;
            }
            if !skipping {
                filtered.push(line);
            }
        }

        let remaining = filtered.join("\n").trim().to_string();
        if remaining.is_empty() || remaining == "#!/bin/sh" {
            let _ = fs::remove_file(&file_path);
        } else {
            fs::write(&file_path, format!("{}\n", remaining))
                .with_context(|| format!("Writing {}", file_path.display()))?;
        }

        uninstalled.push(name.to_string());
    }

    Ok(uninstalled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_hooks_install_and_uninstall() {
        let temp = std::env::temp_dir().join("codeatlas_test_git_hooks");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(temp.join(".git")).unwrap();

        let installed = install_git_hooks(&temp).unwrap();
        assert!(installed.contains(&"post-commit".to_string()));
        assert!(temp.join(".git/hooks/post-commit").exists());

        // Re-install is idempotent
        let reinstalled = install_git_hooks(&temp).unwrap();
        assert!(reinstalled.iter().any(|h| h.contains("already present")));

        let uninstalled = uninstall_git_hooks(&temp).unwrap();
        assert!(uninstalled.contains(&"post-commit".to_string()));
        assert!(!temp.join(".git/hooks/post-commit").exists());

        let _ = fs::remove_dir_all(&temp);
    }
}

