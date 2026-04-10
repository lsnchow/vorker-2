use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use vorker_tui::PromptHistoryStore;

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("vorker-history-{name}-{suffix}"))
}

#[test]
fn prompt_history_persists_unique_recent_prompts() {
    let root = unique_temp_dir("store");
    fs::create_dir_all(&root).expect("create root");
    let path = root.join("prompt-history.jsonl");

    let mut store = PromptHistoryStore::open_at(path.clone()).expect("open store");
    store.append("first prompt").expect("append first");
    store.append("second prompt").expect("append second");
    store.append("first prompt").expect("dedupe first");

    let store = PromptHistoryStore::open_at(path).expect("reload store");
    let recent = store.recent(5);

    assert_eq!(
        recent
            .iter()
            .map(|entry| entry.text.as_str())
            .collect::<Vec<_>>(),
        vec!["first prompt", "second prompt"]
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn prompt_history_ignores_empty_prompts() {
    let root = unique_temp_dir("empty");
    fs::create_dir_all(&root).expect("create root");
    let path = root.join("prompt-history.jsonl");

    let mut store = PromptHistoryStore::open_at(path.clone()).expect("open store");
    store.append("   ").expect("append empty");

    let store = PromptHistoryStore::open_at(path).expect("reload store");
    assert!(store.recent(5).is_empty());

    fs::remove_dir_all(root).ok();
}
