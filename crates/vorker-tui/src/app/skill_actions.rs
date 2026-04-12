use super::*;

pub(crate) fn apply_skill_listing(app: &mut App) {
    if app.skills.is_empty() {
        app.apply_system_notice("No skills found.");
        return;
    }

    app.apply_system_notice("Skills:");
    for skill in app.skills.clone() {
        let marker = if app.enabled_skills.contains(&skill.name) {
            "[x]"
        } else {
            "[ ]"
        };
        app.apply_system_notice(format!(
            "{marker} {}  [Skill] {}",
            skill.name, skill.description
        ));
    }
    app.apply_system_notice(
        "Use /skills enable <name>, /skills disable <name>, or /skills toggle <name>.",
    );
}

pub(crate) fn resolve_skill_name(skills: &[SkillInfo], requested: &str) -> Option<String> {
    let requested = requested.trim();
    if requested.is_empty() {
        return None;
    }

    skills
        .iter()
        .find(|skill| skill.name == requested)
        .or_else(|| {
            let lower = requested.to_ascii_lowercase();
            skills
                .iter()
                .find(|skill| skill.name.to_ascii_lowercase() == lower)
        })
        .or_else(|| {
            let lower = requested.to_ascii_lowercase();
            let mut matches = skills
                .iter()
                .filter(|skill| skill.name.to_ascii_lowercase().contains(&lower));
            let first = matches.next()?;
            matches.next().is_none().then_some(first)
        })
        .map(|skill| skill.name.clone())
}
