use std::fs;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposerMentionBinding {
    pub token: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MentionContext {
    pub sections: Vec<String>,
    pub errors: Vec<String>,
}

#[must_use]
pub fn extract_active_mention_query(buffer: &str) -> Option<String> {
    let trimmed = buffer.trim_end_matches(char::is_whitespace);
    let at_index = trimmed.rfind('@')?;
    let prefix = &trimmed[..at_index];
    if prefix.chars().last().is_some_and(|ch| !ch.is_whitespace()) {
        return None;
    }

    let query = &trimmed[at_index + 1..];
    if query.chars().any(char::is_whitespace) {
        return None;
    }

    Some(query.to_string())
}

#[must_use]
pub fn filter_mention_items(query: &str, paths: &[String]) -> Vec<String> {
    let query = query.to_ascii_lowercase();
    let mut scored: Vec<(usize, usize, String)> = paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let lower = path.to_ascii_lowercase();
            let basename = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
            let score = if query.is_empty() {
                0
            } else if basename.starts_with(&query) {
                0
            } else if lower.contains(&query) {
                1
            } else {
                2
            };
            (score, index, path.clone())
        })
        .collect();

    scored.sort_by(|left, right| left.cmp(right));
    scored.into_iter().map(|(_, _, path)| path).take(8).collect()
}

#[must_use]
pub fn insert_selected_mention(
    buffer: &str,
    selected_path: &str,
) -> Option<(String, ComposerMentionBinding)> {
    let query = extract_active_mention_query(buffer)?;
    let suffix = format!("@{query}");
    let replacement = format!("@{selected_path}");
    let updated = buffer.trim_end_matches(&suffix).to_string() + &replacement + " ";

    Some((
        updated,
        ComposerMentionBinding {
            token: replacement,
            path: selected_path.to_string(),
        },
    ))
}

#[must_use]
pub fn prune_mention_bindings(
    buffer: &str,
    bindings: &[ComposerMentionBinding],
) -> Vec<ComposerMentionBinding> {
    bindings
        .iter()
        .filter(|binding| buffer.contains(&binding.token))
        .cloned()
        .collect()
}

#[must_use]
pub fn resolve_mention_context(
    cwd: &Path,
    bindings: &[ComposerMentionBinding],
) -> MentionContext {
    let mut sections = Vec::new();
    let mut errors = Vec::new();

    for binding in bindings {
        let path = cwd.join(&binding.path);
        match fs::read(&path) {
            Ok(bytes) => {
                if bytes.contains(&0) {
                    errors.push(format!("{} looks like a binary file and was skipped.", binding.path));
                    continue;
                }

                match String::from_utf8(bytes) {
                    Ok(text) => {
                        sections.push(format!("Attached file: {}\n```text\n{}\n```", binding.path, text.trim_end()));
                    }
                    Err(_) => {
                        errors.push(format!("{} could not be decoded as UTF-8 and was skipped.", binding.path));
                    }
                }
            }
            Err(error) => {
                errors.push(format!("{} could not be read: {error}", binding.path));
            }
        }
    }

    MentionContext { sections, errors }
}
