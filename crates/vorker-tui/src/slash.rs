#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlashCommandId {
    Review,
    Ralph,
    Stop,
    Steer,
    Queue,
    Agent,
    Agents,
    AgentStop,
    AgentResult,
    Theme,
    Export,
    Copy,
    Diff,
    Status,
    History,
    Skills,
    Coach,
    Apply,
    ExitReview,
    Model,
    New,
    Help,
    Permissions,
    Rename,
    List,
    Cd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlashCommandCategory {
    Session,
    Review,
    Agent,
    Workflow,
    Config,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlashCommandVisibility {
    pub visible_in_review_mode: bool,
    pub visible_in_normal_mode: bool,
    pub allow_while_busy: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlashCommand {
    pub id: SlashCommandId,
    pub name: &'static str,
    pub description: &'static str,
    pub aliases: &'static [&'static str],
    pub category: SlashCommandCategory,
    pub visibility: SlashCommandVisibility,
}

#[must_use]
pub fn category_label(category: SlashCommandCategory) -> &'static str {
    match category {
        SlashCommandCategory::Session => "Session",
        SlashCommandCategory::Review => "Review",
        SlashCommandCategory::Agent => "Agent",
        SlashCommandCategory::Workflow => "Workflow",
        SlashCommandCategory::Config => "Config",
    }
}

const NORMAL_ONLY: SlashCommandVisibility = SlashCommandVisibility {
    visible_in_review_mode: false,
    visible_in_normal_mode: true,
    allow_while_busy: false,
};

const NORMAL_BUSY: SlashCommandVisibility = SlashCommandVisibility {
    visible_in_review_mode: false,
    visible_in_normal_mode: true,
    allow_while_busy: true,
};

const REVIEW_ONLY: SlashCommandVisibility = SlashCommandVisibility {
    visible_in_review_mode: true,
    visible_in_normal_mode: false,
    allow_while_busy: true,
};

const SHARED: SlashCommandVisibility = SlashCommandVisibility {
    visible_in_review_mode: true,
    visible_in_normal_mode: true,
    allow_while_busy: true,
};

pub const SLASH_COMMANDS: [SlashCommand; 26] = [
    SlashCommand {
        id: SlashCommandId::Review,
        name: "/review",
        description: "run adversarial review; --coach teaches, --apply patches, --staged reviews staged files",
        aliases: &[],
        category: SlashCommandCategory::Review,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Ralph,
        name: "/ralph",
        description: "launch a RALPH persistence session",
        aliases: &[],
        category: SlashCommandCategory::Workflow,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Stop,
        name: "/stop",
        description: "stop the active prompt or review job",
        aliases: &["/clean"],
        category: SlashCommandCategory::Workflow,
        visibility: SHARED,
    },
    SlashCommand {
        id: SlashCommandId::Steer,
        name: "/steer",
        description: "send steering guidance to the active agent",
        aliases: &[],
        category: SlashCommandCategory::Workflow,
        visibility: NORMAL_BUSY,
    },
    SlashCommand {
        id: SlashCommandId::Queue,
        name: "/queue",
        description: "queue a prompt, or use /queue list, /queue pop, and /queue clear",
        aliases: &[],
        category: SlashCommandCategory::Workflow,
        visibility: NORMAL_BUSY,
    },
    SlashCommand {
        id: SlashCommandId::Agent,
        name: "/agent",
        description: "spawn a Codex-backed side agent",
        aliases: &[],
        category: SlashCommandCategory::Agent,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Agents,
        name: "/agents",
        description: "list Codex side agents",
        aliases: &[],
        category: SlashCommandCategory::Agent,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::AgentStop,
        name: "/agent-stop",
        description: "stop a running Codex side agent",
        aliases: &[],
        category: SlashCommandCategory::Agent,
        visibility: NORMAL_BUSY,
    },
    SlashCommand {
        id: SlashCommandId::AgentResult,
        name: "/agent-result",
        description: "show side agent result",
        aliases: &[],
        category: SlashCommandCategory::Agent,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Theme,
        name: "/theme",
        description: "change shell theme",
        aliases: &[],
        category: SlashCommandCategory::Config,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Export,
        name: "/export",
        description: "export the current transcript to markdown",
        aliases: &[],
        category: SlashCommandCategory::Session,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Copy,
        name: "/copy",
        description: "copy the current transcript to the clipboard",
        aliases: &[],
        category: SlashCommandCategory::Session,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Diff,
        name: "/diff",
        description: "show the current working tree diff",
        aliases: &[],
        category: SlashCommandCategory::Session,
        visibility: NORMAL_BUSY,
    },
    SlashCommand {
        id: SlashCommandId::Status,
        name: "/status",
        description: "show session, workspace, and agent status",
        aliases: &[],
        category: SlashCommandCategory::Session,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::History,
        name: "/history",
        description: "show recent prompt history",
        aliases: &[],
        category: SlashCommandCategory::Session,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Skills,
        name: "/skills",
        description: "list, enable, or disable agent skills",
        aliases: &["/$"],
        category: SlashCommandCategory::Config,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Coach,
        name: "/coach",
        description: "rerun review with teaching guidance",
        aliases: &[],
        category: SlashCommandCategory::Review,
        visibility: REVIEW_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Apply,
        name: "/apply",
        description: "rerun review and apply the smallest safe patch",
        aliases: &[],
        category: SlashCommandCategory::Review,
        visibility: REVIEW_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::ExitReview,
        name: "/exit-review",
        description: "leave the review window",
        aliases: &[],
        category: SlashCommandCategory::Review,
        visibility: REVIEW_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Model,
        name: "/model",
        description: "choose what model to use",
        aliases: &[],
        category: SlashCommandCategory::Config,
        visibility: SHARED,
    },
    SlashCommand {
        id: SlashCommandId::New,
        name: "/new",
        description: "start a fresh chat",
        aliases: &[],
        category: SlashCommandCategory::Session,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Help,
        name: "/help",
        description: "show the available shell commands",
        aliases: &["/?"],
        category: SlashCommandCategory::Session,
        visibility: SHARED,
    },
    SlashCommand {
        id: SlashCommandId::Permissions,
        name: "/permissions",
        description: "toggle manual vs auto approvals",
        aliases: &["/approvals"],
        category: SlashCommandCategory::Config,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Rename,
        name: "/rename",
        description: "rename the current thread",
        aliases: &[],
        category: SlashCommandCategory::Session,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::List,
        name: "/list",
        description: "list or reopen saved threads",
        aliases: &[],
        category: SlashCommandCategory::Session,
        visibility: NORMAL_ONLY,
    },
    SlashCommand {
        id: SlashCommandId::Cd,
        name: "/cd",
        description: "change the project directory",
        aliases: &[],
        category: SlashCommandCategory::Session,
        visibility: NORMAL_ONLY,
    },
];

impl SlashCommand {
    #[must_use]
    pub fn matches_exact(self, candidate: &str) -> bool {
        self.name == candidate || self.aliases.iter().any(|alias| *alias == candidate)
    }

    #[must_use]
    pub fn matches_prefix(self, query: &str) -> bool {
        let query = query.to_ascii_lowercase();
        self.name
            .trim_start_matches('/')
            .to_ascii_lowercase()
            .starts_with(&query)
            || self.aliases.iter().any(|alias| {
                alias
                    .trim_start_matches('/')
                    .to_ascii_lowercase()
                    .starts_with(&query)
            })
    }
}

#[must_use]
pub fn is_slash_mode(buffer: &str) -> bool {
    buffer.starts_with('/')
}

#[must_use]
pub fn filtered_commands(buffer: &str, review_mode: bool) -> Vec<SlashCommand> {
    filtered_commands_for_state(buffer, review_mode, false)
}

#[must_use]
pub fn filtered_commands_for_state(
    buffer: &str,
    review_mode: bool,
    busy: bool,
) -> Vec<SlashCommand> {
    if !is_slash_mode(buffer) {
        return Vec::new();
    }

    let query = buffer
        .trim_start_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();

    let commands = SLASH_COMMANDS
        .into_iter()
        .filter(|command| {
            if review_mode {
                command.visibility.visible_in_review_mode
            } else {
                command.visibility.visible_in_normal_mode
            }
        })
        .filter(|command| !busy || command.visibility.allow_while_busy)
        .collect::<Vec<_>>();

    if query.is_empty() {
        return commands;
    }

    commands
        .iter()
        .copied()
        .filter(|command| command.matches_prefix(&query))
        .collect()
}

#[must_use]
pub fn help_summary(review_mode: bool, busy: bool) -> String {
    let commands = SLASH_COMMANDS
        .into_iter()
        .filter(|command| {
            if review_mode {
                command.visibility.visible_in_review_mode
            } else {
                command.visibility.visible_in_normal_mode
            }
        })
        .filter(|command| !busy || command.visibility.allow_while_busy)
        .map(|command| command.name)
        .collect::<Vec<_>>();

    format!("Commands: {}", commands.join(" "))
}
