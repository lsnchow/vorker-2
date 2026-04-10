use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use vorker_tui::{SkillStore, build_skill_context, discover_skills};

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("vorker-skills-{name}-{suffix}"))
}

#[test]
fn discover_skills_reads_frontmatter() {
    let root = unique_temp_dir("discover");
    let skill_dir = root.join("skills").join("review");
    fs::create_dir_all(&skill_dir).expect("skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: code-review\ndescription: Review code carefully\n---\nBody\n",
    )
    .expect("skill");

    let skills = discover_skills(&[root.join("skills")]).expect("discover");

    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "code-review");
    assert_eq!(skills[0].description, "Review code carefully");
    assert!(skills[0].path.ends_with("SKILL.md"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn skill_store_toggles_enabled_skills() {
    let root = unique_temp_dir("store");
    fs::create_dir_all(&root).expect("root");
    let path = root.join("skills.json");

    let mut store = SkillStore::open_at(path.clone()).expect("store");
    assert!(!store.is_enabled("code-review"));

    store.set_enabled("code-review", true).expect("enable");
    assert!(store.is_enabled("code-review"));

    let store = SkillStore::open_at(path).expect("reload");
    assert!(store.is_enabled("code-review"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_skill_context_renders_enabled_skill_instructions() {
    let root = unique_temp_dir("context");
    let skill_dir = root.join("skills").join("review");
    fs::create_dir_all(&skill_dir).expect("skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: code-review\ndescription: Review code carefully\n---\n# Review\nRead the diff and report bugs.\n",
    )
    .expect("skill");
    let skills = discover_skills(&[root.join("skills")]).expect("discover");
    let enabled = ["code-review".to_string()].into_iter().collect();

    let context = build_skill_context(&skills, &enabled).expect("context");

    assert!(context.contains("Enabled Vorker skills"));
    assert!(context.contains("## code-review"));
    assert!(context.contains("Read the diff and report bugs."));

    fs::remove_dir_all(root).ok();
}
