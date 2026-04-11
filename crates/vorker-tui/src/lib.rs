//! Native Rust TUI support for Vorker.

mod app;
mod boot;
mod bridge;
mod demo;
mod mentions;
mod navigation;
mod project_workspace;
mod prompt_history;
mod render;
mod rich_text;
mod side_agent_store;
mod skill_store;
mod slash;
mod theme;
mod thread_store;
mod transcript_export;

pub use app::{App, AppCommand, PermissionOptionView, render_once, run_app};
pub use boot::{BootStep, boot_minimum_ticks, render_boot_frame};
pub use demo::render_hyperloop_mock;
pub use mentions::{
    ComposerMentionBinding, MentionContext, collect_buffer_mentions, extract_active_mention_query,
    filter_mention_items, insert_selected_mention, prune_mention_bindings, resolve_mention_context,
};
pub use navigation::{
    ACTION_ITEMS, ActionItem, NavKey, NavigationState, Pane, apply_navigation_key,
    reconcile_navigation_state,
};
pub use project_workspace::{ProjectWorkspace, render_project_confirmation};
pub use prompt_history::{PromptHistoryEntry, PromptHistoryStore};
pub use render::{DashboardOptions, PopupItem, RowKind, TranscriptRow, render_dashboard};
pub use side_agent_store::{
    SideAgentStatus, SideAgentStore, StoredSideAgentJob, summarize_side_agent_events,
};
pub use skill_store::{SkillInfo, SkillStore, build_skill_context, discover_skills};
pub use slash::{
    SlashCommand, SlashCommandCategory, SlashCommandId, SlashCommandVisibility, category_label,
    filtered_commands, filtered_commands_for_state, help_summary, is_slash_mode,
};
pub use thread_store::{ApprovalMode, StoredThread, ThreadStore};
pub use transcript_export::{render_transcript_markdown, write_transcript_export};
