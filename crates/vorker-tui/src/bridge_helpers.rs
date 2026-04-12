use crate::bridge::PermissionOption;

pub fn tool_update_text(title: Option<String>, detail: Option<String>) -> Option<String> {
    detail
        .filter(|detail| !detail.trim().is_empty())
        .or_else(|| title.filter(|title| !title.trim().is_empty()))
}

pub fn choose_auto_permission(options: &[PermissionOption]) -> Option<PermissionOption> {
    let mut ranked = options.to_vec();
    ranked.sort_by_key(|option| match option.kind.as_str() {
        "allow_always" => 0,
        "allow_once" => 1,
        "reject_once" => 2,
        "reject_always" => 3,
        _ => 4,
    });
    ranked.into_iter().next()
}
