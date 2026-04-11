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
pub struct SlashCommand {
    pub id: SlashCommandId,
    pub name: &'static str,
    pub description: &'static str,
    pub aliases: &'static [&'static str],
}

pub const SLASH_COMMANDS: [SlashCommand; 24] = [
    SlashCommand {
        id: SlashCommandId::Review,
        name: "/review",
        description: "run adversarial review; --coach teaches, --apply patches, --staged reviews staged files",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::Ralph,
        name: "/ralph",
        description: "launch a RALPH persistence session",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::Stop,
        name: "/stop",
        description: "stop the active prompt or review job",
        aliases: &["/clean"],
    },
    SlashCommand {
        id: SlashCommandId::Steer,
        name: "/steer",
        description: "send steering guidance to the active agent",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::Queue,
        name: "/queue",
        description: "queue a prompt, or use /queue list, /queue pop, and /queue clear",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::Agent,
        name: "/agent",
        description: "spawn a Codex-backed side agent",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::Agents,
        name: "/agents",
        description: "list Codex side agents",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::AgentStop,
        name: "/agent-stop",
        description: "stop a running Codex side agent",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::AgentResult,
        name: "/agent-result",
        description: "show side agent result",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::Theme,
        name: "/theme",
        description: "change shell theme",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::Export,
        name: "/export",
        description: "export the current transcript to markdown",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::Status,
        name: "/status",
        description: "show session, workspace, and agent status",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::History,
        name: "/history",
        description: "show recent prompt history",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::Skills,
        name: "/skills",
        description: "list, enable, or disable agent skills",
        aliases: &["/$"],
    },
    SlashCommand {
        id: SlashCommandId::Coach,
        name: "/coach",
        description: "rerun review with teaching guidance",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::Apply,
        name: "/apply",
        description: "rerun review and apply the smallest safe patch",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::ExitReview,
        name: "/exit-review",
        description: "leave the review window",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::Model,
        name: "/model",
        description: "choose what model to use",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::New,
        name: "/new",
        description: "start a fresh chat",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::Help,
        name: "/help",
        description: "show the available shell commands",
        aliases: &["/?"],
    },
    SlashCommand {
        id: SlashCommandId::Permissions,
        name: "/permissions",
        description: "toggle manual vs auto approvals",
        aliases: &["/approvals"],
    },
    SlashCommand {
        id: SlashCommandId::Rename,
        name: "/rename",
        description: "rename the current thread",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::List,
        name: "/list",
        description: "list or reopen saved threads",
        aliases: &[],
    },
    SlashCommand {
        id: SlashCommandId::Cd,
        name: "/cd",
        description: "change the project directory",
        aliases: &[],
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
    if !is_slash_mode(buffer) {
        return Vec::new();
    }

    let query = buffer
        .trim_start_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();

    let commands = if review_mode {
        SLASH_COMMANDS
            .into_iter()
            .filter(|command| {
                matches!(
                    command.id,
                    SlashCommandId::Stop
                        | SlashCommandId::Coach
                        | SlashCommandId::Apply
                        | SlashCommandId::ExitReview
                        | SlashCommandId::Model
                )
            })
            .collect()
    } else {
        SLASH_COMMANDS.to_vec()
    };

    if query.is_empty() {
        return commands;
    }

    commands
        .iter()
        .copied()
        .filter(|command| command.matches_prefix(&query))
        .collect()
}
