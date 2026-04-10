use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use vorker_tui::{ComposerMentionBinding, resolve_mention_context};

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
