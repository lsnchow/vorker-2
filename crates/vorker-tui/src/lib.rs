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
mod slash;
mod theme;
mod thread_store;
mod transcript_export;

pub use app::{App, AppCommand, PermissionOptionView, render_once, run_app};
pub use boot::{BootStep, boot_minimum_ticks, render_boot_frame};
pub use demo::render_hyperloop_mock;
pub use mentions::{ComposerMentionBinding, MentionContext, resolve_mention_context};
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
pub use slash::{SlashCommand, SlashCommandId, filtered_commands, is_slash_mode};
pub use thread_store::{ApprovalMode, StoredThread, ThreadStore};
pub use transcript_export::{render_transcript_markdown, write_transcript_export};
