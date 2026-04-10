use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SkillStorePayload {
    enabled: BTreeSet<String>,
}

pub struct SkillStore {
    path: PathBuf,
    enabled: BTreeSet<String>,
}

impl SkillStore {
    pub fn open_at(path: PathBuf) -> io::Result<Self> {
        let enabled = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            if raw.trim().is_empty() {
                BTreeSet::new()
            } else {
                serde_json::from_str::<SkillStorePayload>(&raw)
                    .map(|payload| payload.enabled)
                    .map_err(|error| invalid_data_error(&path, error))?
            }
        } else {
            BTreeSet::new()
        };
        Ok(Self { path, enabled })
    }

    #[must_use]
    pub fn is_enabled(&self, skill_name: &str) -> bool {
        self.enabled.contains(skill_name)
    }

    pub fn set_enabled(&mut self, skill_name: &str, enabled: bool) -> io::Result<()> {
        if enabled {
            self.enabled.insert(skill_name.to_string());
        } else {
            self.enabled.remove(skill_name);
        }
        self.persist()
    }

    #[must_use]
    pub fn enabled(&self) -> BTreeSet<String> {
        self.enabled.clone()
    }

    fn persist(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let payload = SkillStorePayload {
            enabled: self.enabled.clone(),
        };
        let data = serde_json::to_string_pretty(&payload).map_err(io::Error::other)?;
        fs::write(&self.path, data)
    }
}

pub fn discover_skills(roots: &[PathBuf]) -> io::Result<Vec<SkillInfo>> {
    let mut skills = Vec::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path().join("SKILL.md");
            if !path.exists() {
                continue;
            }
            let source = fs::read_to_string(&path)?;
            let (name, description) =
                parse_skill_frontmatter(&source).unwrap_or_else(|| fallback_skill_info(&path));
            if !seen.insert(name.clone()) {
                continue;
            }
            skills.push(SkillInfo {
                name,
                description,
                path,
            });
        }
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(skills)
}

pub fn build_skill_context(skills: &[SkillInfo], enabled: &BTreeSet<String>) -> io::Result<String> {
    if enabled.is_empty() {
        return Ok(String::new());
    }

    let mut sections = Vec::new();
    for skill in skills.iter().filter(|skill| enabled.contains(&skill.name)) {
        let source = fs::read_to_string(&skill.path)?;
        let excerpt = skill_body_excerpt(&source, 3_500);
        sections.push(format!(
            "## {}\nDescription: {}\nPath: {}\n\n{}",
            skill.name,
            skill.description,
            skill.path.display(),
            excerpt
        ));
    }

    if sections.is_empty() {
        return Ok(String::new());
    }

    Ok(format!(
        "Enabled Vorker skills:\n\n{}",
        sections.join("\n\n---\n\n")
    ))
}

fn parse_skill_frontmatter(source: &str) -> Option<(String, String)> {
    let body = source.strip_prefix("---\n")?;
    let (frontmatter, _) = body.split_once("\n---")?;
    let mut name = None;
    let mut description = None;
    for line in frontmatter.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        match key.trim() {
            "name" => name = Some(value),
            "description" => description = Some(value),
            _ => {}
        }
    }
    Some((name?, description.unwrap_or_else(|| "Skill".to_string())))
}

fn fallback_skill_info(path: &Path) -> (String, String) {
    let name = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .unwrap_or("skill")
        .to_string();
    (name, "Skill".to_string())
}

fn skill_body_excerpt(source: &str, max_chars: usize) -> String {
    let body = source
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---").map(|(_, body)| body.trim()))
        .unwrap_or(source)
        .trim();
    let mut excerpt = body.chars().take(max_chars).collect::<String>();
    if body.chars().count() > max_chars {
        excerpt.push_str("\n\n[skill instructions truncated]");
    }
    excerpt
}

fn invalid_data_error(path: &Path, error: serde_json::Error) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("failed to parse {}: {error}", path.display()),
    )
}
