#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlashCommandId {
    Model,
    Provider,
    New,
    Agents,
    Runs,
    Tasks,
    Review,
    Permissions,
    Share,
    Preflight,
    Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlashCommand {
    pub id: SlashCommandId,
    pub name: &'static str,
    pub description: &'static str,
}

pub const SLASH_COMMANDS: [SlashCommand; 11] = [
    SlashCommand {
        id: SlashCommandId::Model,
        name: "/model",
        description: "Switch the active model",
    },
    SlashCommand {
        id: SlashCommandId::Provider,
        name: "/provider",
        description: "Switch the active provider",
    },
    SlashCommand {
        id: SlashCommandId::New,
        name: "/new",
        description: "Create a new agent",
    },
    SlashCommand {
        id: SlashCommandId::Agents,
        name: "/agents",
        description: "Open the agents sidebar",
    },
    SlashCommand {
        id: SlashCommandId::Runs,
        name: "/runs",
        description: "Open the runs sidebar",
    },
    SlashCommand {
        id: SlashCommandId::Tasks,
        name: "/tasks",
        description: "Open the tasks sidebar",
    },
    SlashCommand {
        id: SlashCommandId::Review,
        name: "/review",
        description: "Show the review flow",
    },
    SlashCommand {
        id: SlashCommandId::Permissions,
        name: "/permissions",
        description: "Inspect approval mode",
    },
    SlashCommand {
        id: SlashCommandId::Share,
        name: "/share",
        description: "Inspect tunnel/share state",
    },
    SlashCommand {
        id: SlashCommandId::Preflight,
        name: "/preflight",
        description: "Inspect preflight guidance",
    },
    SlashCommand {
        id: SlashCommandId::Help,
        name: "/help",
        description: "Show slash command help",
    },
];

#[must_use]
pub fn is_slash_mode(buffer: &str) -> bool {
    buffer.starts_with('/')
}

#[must_use]
pub fn filtered_commands(buffer: &str) -> Vec<SlashCommand> {
    if !is_slash_mode(buffer) {
        return Vec::new();
    }

    let query = buffer.trim_start_matches('/').trim().to_ascii_lowercase();
    if query.is_empty() {
        return SLASH_COMMANDS.to_vec();
    }

    SLASH_COMMANDS
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
