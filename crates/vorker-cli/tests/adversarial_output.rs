use vorker_cli::adversarial::{
    AdversarialFinding, AdversarialReport, ReviewScope, build_popout_command,
    render_markdown_report,
};

#[test]
fn markdown_report_renders_findings_with_code_blocks_and_coaching() {
    let report = AdversarialReport {
        verdict: "needs-attention".to_string(),
        summary: "The current API shape leaks failure handling into every caller.".to_string(),
        findings: vec![AdversarialFinding {
            severity: "high".to_string(),
            title: "Handler ignores partial failure".to_string(),
            body: "This path swallows an upstream write failure and still returns success."
                .to_string(),
            file: "src/api.rs".to_string(),
            line_start: 24,
            line_end: 30,
            confidence: 0.91,
            recommendation: "Return an explicit error and stop the success path when the write fails."
                .to_string(),
            code_snippet: Some("if write_result.is_err() {\n    return Ok(());\n}".to_string()),
            teaching_note: Some(
                "A good API should make failure explicit instead of forcing reviewers to infer hidden failure states."
                    .to_string(),
            ),
            patch_plan: Some(
                "Refactor the handler to return Result<Response, ApiError> and propagate the write failure."
                    .to_string(),
            ),
        }],
        next_steps: vec!["Fix the error path before shipping.".to_string()],
    };

    let markdown = render_markdown_report(&report, true);

    assert!(markdown.contains("# Adversarial Review"));
    assert!(markdown.contains("## Findings"));
    assert!(markdown.contains("```rust"));
    assert!(markdown.contains("if write_result.is_err()"));
    assert!(markdown.contains("## Coaching"));
    assert!(markdown.contains("good API should make failure explicit"));
    assert!(markdown.contains("## Suggested Patch Direction"));
}

#[test]
fn popout_command_targets_red_gpt_review_shell() {
    let command = build_popout_command(
        "/Users/lucas/Downloads",
        "gpt-5.3-codex",
        ReviewScope::WorkingTree,
        true,
        false,
        "review the safety code",
    );

    assert!(command.contains("VORKER_THEME=review"));
    assert!(command.contains("VORKER_REVIEW_MODE=1"));
    assert!(command.contains("VORKER_REVIEW_SCOPE=working-tree"));
    assert!(command.contains("VORKER_REVIEW_COACH=1"));
    assert!(command.contains("VORKER_REVIEW_FOCUS='review the safety code'"));
    assert!(command.contains("--model gpt-5.3-codex"));
    assert!(command.contains("/Users/lucas/Downloads"));
}
