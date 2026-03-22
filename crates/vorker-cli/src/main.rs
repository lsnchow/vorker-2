use clap::{Args, CommandFactory, Parser, Subcommand};
use std::env;
use std::io::{self, Read};
use vorker_agent::{PromptRequest, ProviderId, ProviderManager};
use vorker_core::EventLog;
use vorker_preflight::{LocalContainerSandbox, PreflightRequest, PreflightRunner};
use vorker_tui::{render_once, run_app};

#[derive(Debug, Parser)]
#[command(
    name = "vorker",
    about = "Rust-native Vorker runtime",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(flatten)]
    shared: SharedOptions,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Args, Default)]
struct SharedOptions {
    #[arg(long)]
    cwd: Option<String>,
    #[arg(long)]
    provider: Option<String>,
    #[arg(long = "copilot-bin")]
    copilot_bin: Option<String>,
    #[arg(long = "codex-bin")]
    codex_bin: Option<String>,
    #[arg(long)]
    mode: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long = "auto-approve", default_value_t = false)]
    auto_approve: bool,
    #[arg(long, default_value_t = false)]
    debug: bool,
    #[arg(long = "no-alt-screen", default_value_t = false)]
    no_alt_screen: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    Tui(TuiOptions),
    Preflight { repo: String },
    Repl,
    Chat { prompt: Option<String> },
    Serve(ServeOptions),
    Share(ShareOptions),
    Help,
}

#[derive(Debug, Args, Default)]
struct TuiOptions {
    #[arg(long, default_value_t = false)]
    once: bool,
}

#[derive(Debug, Args, Default)]
struct ServeOptions {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 4173)]
    port: u16,
    #[arg(long = "tls-key")]
    tls_key: Option<String>,
    #[arg(long = "tls-cert")]
    tls_cert: Option<String>,
    #[arg(long = "trust-proxy", default_value_t = false)]
    trust_proxy: bool,
    #[arg(long = "allow-insecure-http", default_value_t = false)]
    allow_insecure_http: bool,
}

#[derive(Debug, Args, Default)]
struct ShareOptions {
    #[arg(long = "cloudflared-bin")]
    cloudflared_bin: Option<String>,
    #[arg(long = "cloudflared-protocol")]
    cloudflared_protocol: Option<String>,
    #[arg(long = "cloudflared-edge-ip-version")]
    cloudflared_edge_ip_version: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    if let Some(cwd) = &cli.shared.cwd
        && let Err(error) = env::set_current_dir(cwd)
    {
        eprintln!("failed to change directory to {cwd}: {error}");
        std::process::exit(1);
    }

    match cli.command {
        Some(Command::Tui(tui)) => {
            if tui.once {
                println!("{}", render_once(120));
            } else if let Err(error) = run_app(cli.shared.no_alt_screen) {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        Some(Command::Preflight { repo }) => {
            if let Err(error) = run_preflight(repo, cli.shared.auto_approve) {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        Some(Command::Repl) => {
            println!("Rust REPL bootstrap not wired yet.");
        }
        Some(Command::Chat { prompt }) => {
            if let Err(error) = run_chat(prompt, &cli.shared) {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        Some(Command::Serve(_)) => {
            println!("Rust server bootstrap not wired yet.");
        }
        Some(Command::Share(_)) => {
            println!("Rust share bootstrap not wired yet.");
        }
        Some(Command::Help) | None => {
            let _ = Cli::command().print_help();
            println!();
        }
    }
}

fn run_preflight(
    repo: String,
    auto_approve: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let runner = PreflightRunner::new(LocalContainerSandbox::detect());
    let result = runner.run(PreflightRequest::new(repo).approve_high_risk(auto_approve))?;

    let logs_root = env::current_dir()?.join(".vorker-2").join("logs");
    let event_log = EventLog::new(&logs_root, Some(logs_root.join("supervisor.ndjson")));
    for event in &result.events {
        event_log.append(event)?;
    }

    println!("preflight {}", result.report.run_id);
    println!("outcome   {}", result.report.outcome);
    println!("class     {}", result.report.repo_class);
    println!("risk      {}", result.report.risk.level);
    println!("stage     {}", result.report.stage);
    if let Some(failure) = &result.report.latest_failure {
        println!("failure   {failure}");
    }
    println!("summary   {}", result.report.summary_path);
    println!("report    {}", result.report.report_path);
    println!("artifacts {}", result.artifacts_dir.display());
    if result.report.risk.level == "high" && !auto_approve {
        println!(
            "hint      rerun with --auto-approve to allow sandbox execution for a high-risk repo"
        );
    }
    Ok(())
}

fn run_chat(
    prompt: Option<String>,
    shared: &SharedOptions,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let provider = shared
        .provider
        .as_deref()
        .unwrap_or("copilot")
        .parse::<ProviderId>()
        .map_err(io::Error::other)?;
    let prompt = resolve_prompt(prompt)?;
    let request = PromptRequest {
        prompt,
        cwd: Some(env::current_dir()?),
        model: shared.model.clone(),
    };
    let mut spec = ProviderManager::build_prompt_command(provider, &request);
    match provider {
        ProviderId::Copilot => {
            if let Some(bin) = &shared.copilot_bin {
                spec.program = bin.clone();
            }
        }
        ProviderId::Codex => {
            if let Some(bin) = &shared.codex_bin {
                spec.program = bin.clone();
            }
        }
    }

    let output = spec.command().output()?;
    if !output.stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }
    if !output.status.success() {
        return Err(
            io::Error::other(format!("{} exited with status {}", provider, output.status)).into(),
        );
    }

    Ok(())
}

fn resolve_prompt(
    prompt: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(prompt) = prompt.filter(|value| !value.trim().is_empty()) {
        return Ok(prompt);
    }

    let mut stdin = String::new();
    io::stdin().read_to_string(&mut stdin)?;
    if stdin.trim().is_empty() {
        return Err(io::Error::other("chat requires a prompt").into());
    }
    Ok(stdin)
}
