#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlashCommandId {
    Review,
    Stop,
    Steer,
    Queue,
    Agent,
    Theme,
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
}

pub const SLASH_COMMANDS: [SlashCommand; 16] = [
    SlashCommand {
        id: SlashCommandId::Review,
        name: "/review",
        description: "run adversarial review; --coach teaches, --apply patches, --staged reviews staged files",
    },
    SlashCommand {
        id: SlashCommandId::Stop,
        name: "/stop",
        description: "stop the active prompt or review job",
    },
    SlashCommand {
        id: SlashCommandId::Steer,
        name: "/steer",
        description: "send steering guidance to the active agent",
    },
    SlashCommand {
        id: SlashCommandId::Queue,
        name: "/queue",
        description: "queue a follow-up prompt after current work finishes",
    },
    SlashCommand {
        id: SlashCommandId::Agent,
        name: "/agent",
        description: "spawn a Codex-backed side agent",
    },
    SlashCommand {
        id: SlashCommandId::Theme,
        name: "/theme",
        description: "change shell theme",
    },
    SlashCommand {
        id: SlashCommandId::Coach,
        name: "/coach",
        description: "rerun review with teaching guidance",
    },
    SlashCommand {
        id: SlashCommandId::Apply,
        name: "/apply",
        description: "rerun review and apply the smallest safe patch",
    },
    SlashCommand {
        id: SlashCommandId::ExitReview,
        name: "/exit-review",
        description: "leave the review window",
    },
    SlashCommand {
        id: SlashCommandId::Model,
        name: "/model",
        description: "choose what model to use",
    },
    SlashCommand {
        id: SlashCommandId::New,
        name: "/new",
        description: "start a fresh chat",
    },
    SlashCommand {
        id: SlashCommandId::Help,
        name: "/help",
        description: "show the available shell commands",
    },
    SlashCommand {
        id: SlashCommandId::Permissions,
        name: "/permissions",
        description: "toggle manual vs auto approvals",
    },
    SlashCommand {
        id: SlashCommandId::Rename,
        name: "/rename",
        description: "rename the current thread",
    },
    SlashCommand {
        id: SlashCommandId::List,
        name: "/list",
        description: "list or reopen saved threads",
    },
    SlashCommand {
        id: SlashCommandId::Cd,
        name: "/cd",
        description: "change the project directory",
    },
];

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
        vec![
            SLASH_COMMANDS[1],
            SLASH_COMMANDS[6],
            SLASH_COMMANDS[7],
            SLASH_COMMANDS[8],
            SLASH_COMMANDS[9],
        ]
    } else {
        SLASH_COMMANDS.to_vec()
    };

    if query.is_empty() {
        return commands;
    }

    commands
        .iter()
        .copied()
        .filter(|command| {
            command
                .name
                .trim_start_matches('/')
                .to_ascii_lowercase()
                .starts_with(&query)
        })
        .collect()
}
