//! Native Rust TUI support for Vorker.

mod app;
mod boot;
mod navigation;
mod render;
mod theme;

pub use app::{App, render_once, run_app};
pub use boot::{BootStep, render_boot_frame};
pub use navigation::{
    ACTION_ITEMS, ActionItem, NavKey, NavigationState, Pane, apply_navigation_key,
    reconcile_navigation_state,
};
pub use render::{DashboardOptions, InputMode, render_dashboard};
