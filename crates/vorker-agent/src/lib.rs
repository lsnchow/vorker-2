use std::path::PathBuf;
use std::process::{Command, Output};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Copilot,
    Codex,
}

impl ProviderId {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Copilot => "copilot",
            Self::Codex => "codex",
        }
    }

    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Copilot => "Copilot",
            Self::Codex => "Codex",
        }
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ProviderId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "copilot" => Ok(Self::Copilot),
            "codex" => Ok(Self::Codex),
            other => Err(format!("unknown provider: {other}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptRequest {
    pub prompt: String,
    pub cwd: Option<PathBuf>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

impl CommandSpec {
    #[must_use]
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        command
    }
}

pub trait AgentProvider {
    fn id(&self) -> ProviderId;
    fn default_model(&self) -> &'static str;
    fn binary_name(&self) -> &'static str;
    fn build_prompt_command(&self, request: &PromptRequest) -> CommandSpec;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CopilotProvider;

impl AgentProvider for CopilotProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Copilot
    }

    fn default_model(&self) -> &'static str {
        "gpt-5.4"
    }

    fn binary_name(&self) -> &'static str {
        "copilot"
    }

    fn build_prompt_command(&self, request: &PromptRequest) -> CommandSpec {
        let mut args = vec![
            "--output-format".to_string(),
            "json".to_string(),
            "--allow-all-tools".to_string(),
            "--prompt".to_string(),
            request.prompt.clone(),
        ];
        if let Some(model) = &request.model {
            args.splice(0..0, ["--model".to_string(), model.clone()]);
        }
        if let Some(cwd) = &request.cwd {
            args.splice(0..0, ["--add-dir".to_string(), cwd.display().to_string()]);
        }
        CommandSpec {
            program: self.binary_name().to_string(),
            args,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CodexProvider;

impl AgentProvider for CodexProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Codex
    }

    fn default_model(&self) -> &'static str {
        "gpt-5.4"
    }

    fn binary_name(&self) -> &'static str {
        "codex"
    }

    fn build_prompt_command(&self, request: &PromptRequest) -> CommandSpec {
        let mut args = vec![
            "exec".to_string(),
            "--json".to_string(),
            "--skip-git-repo-check".to_string(),
        ];
        if let Some(model) = &request.model {
            args.extend(["--model".to_string(), model.clone()]);
        }
        if let Some(cwd) = &request.cwd {
            args.extend(["-C".to_string(), cwd.display().to_string()]);
        }
        args.push(request.prompt.clone());
        CommandSpec {
            program: self.binary_name().to_string(),
            args,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProviderManager;

impl ProviderManager {
    #[must_use]
    pub fn available_providers() -> [ProviderId; 2] {
        [ProviderId::Copilot, ProviderId::Codex]
    }

    #[must_use]
    pub fn default_provider() -> ProviderId {
        ProviderId::Copilot
    }

    #[must_use]
    pub fn default_model(provider: ProviderId) -> &'static str {
        match provider {
            ProviderId::Copilot => CopilotProvider.default_model(),
            ProviderId::Codex => CodexProvider.default_model(),
        }
    }

    #[must_use]
    pub fn build_prompt_command(provider: ProviderId, request: &PromptRequest) -> CommandSpec {
        match provider {
            ProviderId::Copilot => CopilotProvider.build_prompt_command(request),
            ProviderId::Codex => CodexProvider.build_prompt_command(request),
        }
    }

    pub fn run_prompt(
        provider: ProviderId,
        request: &PromptRequest,
    ) -> Result<Output, std::io::Error> {
        Self::build_prompt_command(provider, request)
            .command()
            .output()
    }
}

#[cfg(test)]
mod tests {
    use super::{PromptRequest, ProviderId, ProviderManager};
    use std::path::PathBuf;

    #[test]
    fn copilot_provider_builds_a_non_interactive_prompt_command() {
        let spec = ProviderManager::build_prompt_command(
            ProviderId::Copilot,
            &PromptRequest {
                prompt: "Fix the bug".to_string(),
                cwd: Some(PathBuf::from("/repo")),
                model: Some("gpt-5.4".to_string()),
            },
        );

        assert_eq!(spec.program, "copilot");
        assert_eq!(
            spec.args,
            vec![
                "--add-dir",
                "/repo",
                "--model",
                "gpt-5.4",
                "--output-format",
                "json",
                "--allow-all-tools",
                "--prompt",
                "Fix the bug",
            ]
        );
    }

    #[test]
    fn codex_provider_builds_a_non_interactive_exec_command() {
        let spec = ProviderManager::build_prompt_command(
            ProviderId::Codex,
            &PromptRequest {
                prompt: "Explain the repository".to_string(),
                cwd: Some(PathBuf::from("/repo")),
                model: Some("gpt-5.4".to_string()),
            },
        );

        assert_eq!(spec.program, "codex");
        assert_eq!(
            spec.args,
            vec![
                "exec",
                "--json",
                "--skip-git-repo-check",
                "--model",
                "gpt-5.4",
                "-C",
                "/repo",
                "Explain the repository",
            ]
        );
    }
}
