use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use vorker_tui::{
    ComposerMentionBinding, collect_buffer_mentions, filter_mention_items, insert_selected_mention,
    prune_mention_bindings, resolve_mention_context,
};

fn temp_path(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("vorker-tui-{name}-{unique}"))
}

#[test]
fn resolve_mention_context_reads_text_files_and_rejects_binary_files() {
    let root = temp_path("mentions");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("README.md"), "# Hello\nThis is text.\n").expect("write text");
    fs::write(root.join("image.bin"), [0_u8, 159, 146, 150]).expect("write binary");

    let result = resolve_mention_context(
        &root,
        &[
            ComposerMentionBinding {
                token: "@README.md".to_string(),
                path: "README.md".to_string(),
            },
            ComposerMentionBinding {
                token: "@image.bin".to_string(),
                path: "image.bin".to_string(),
            },
        ],
    );

    assert_eq!(result.sections.len(), 1);
    assert!(result.sections[0].contains("README.md"));
    assert!(result.sections[0].contains("This is text"));
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].contains("image.bin"));
    assert!(result.errors[0].to_ascii_lowercase().contains("binary"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn resolve_mention_context_can_attach_only_a_line_range() {
    let root = temp_path("mention-range");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("README.md"), "line 1\nline 2\nline 3\nline 4\n").expect("write text");

    let result = resolve_mention_context(
        &root,
        &[ComposerMentionBinding {
            token: "@README.md#L2-L3".to_string(),
            path: "README.md#L2-L3".to_string(),
        }],
    );

    assert_eq!(result.errors.len(), 0);
    assert_eq!(result.sections.len(), 1);
    assert!(result.sections[0].contains("README.md#L2-L3"));
    assert!(result.sections[0].contains("line 2"));
    assert!(result.sections[0].contains("line 3"));
    assert!(!result.sections[0].contains("line 1"));
    assert!(!result.sections[0].contains("line 4"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn prune_mention_bindings_tracks_line_range_suffixes() {
    let bindings = vec![ComposerMentionBinding {
        token: "@README.md".to_string(),
        path: "README.md".to_string(),
    }];

    let pruned = prune_mention_bindings("Review @README.md#L10-L20 please", &bindings);

    assert_eq!(pruned.len(), 1);
    assert_eq!(pruned[0].token, "@README.md#L10-L20");
    assert_eq!(pruned[0].path, "README.md#L10-L20");
}

#[test]
fn collect_buffer_mentions_includes_manual_mentions_without_existing_bindings() {
    let bindings = collect_buffer_mentions("Review @README.md#L2-L3 please", &[]);

    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].token, "@README.md#L2-L3");
    assert_eq!(bindings[0].path, "README.md#L2-L3");
}

#[test]
fn filter_mention_items_ignores_line_range_suffix_while_matching() {
    let items = filter_mention_items(
        "rea#L10-L20",
        &["zeta.txt".to_string(), "README.md".to_string()],
    );

    assert_eq!(items[0], "README.md");
}

#[test]
fn insert_selected_mention_preserves_typed_line_range_suffix() {
    let (updated, binding) =
        insert_selected_mention("Review @rea#L10-L20", "README.md").expect("mention");

    assert_eq!(updated, "Review @README.md#L10-L20 ");
    assert_eq!(binding.token, "@README.md#L10-L20");
    assert_eq!(binding.path, "README.md#L10-L20");
}
