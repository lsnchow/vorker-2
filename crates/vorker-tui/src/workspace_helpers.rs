use std::io;
use std::path::{Path, PathBuf};

pub fn skill_roots_for(cwd: &Path) -> Vec<PathBuf> {
    let mut roots = vec![
        cwd.join(".codex").join("skills"),
        cwd.join(".agents").join("skills"),
        cwd.join(".github").join("skills"),
    ];

    if let Ok(codex_home) = std::env::var("CODEX_HOME") {
        roots.push(PathBuf::from(codex_home).join("skills"));
    } else if let Some(home) = home_dir() {
        roots.push(home.join(".codex").join("skills"));
        roots.push(home.join(".codex").join("superpowers").join("skills"));
        roots.push(home.join(".agents").join("skills"));
    }

    roots
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn resolve_directory_change(current: &Path, requested: &str) -> io::Result<PathBuf> {
    let candidate = PathBuf::from(requested);
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        current.join(candidate)
    };
    let resolved = resolved.canonicalize()?;
    if !resolved.is_dir() {
        return Err(io::Error::other(format!(
            "{} is not a directory",
            resolved.display()
        )));
    }
    Ok(resolved)
}

pub fn load_workspace_files(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let entry_path = entry.path();
            let Ok(relative) = entry_path.strip_prefix(root) else {
                continue;
            };
            if relative.as_os_str().is_empty() {
                continue;
            }

            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                let skip = relative.iter().any(|segment| {
                    matches!(
                        segment.to_string_lossy().as_ref(),
                        ".git" | "node_modules" | "target" | ".next" | "dist"
                    )
                });
                if !skip {
                    stack.push(entry_path);
                }
                continue;
            }

            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }

    files.sort();
    files
}
