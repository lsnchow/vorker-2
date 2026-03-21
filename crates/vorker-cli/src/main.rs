use clap::{Args, CommandFactory, Parser, Subcommand};
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
    #[arg(long = "copilot-bin")]
    copilot_bin: Option<String>,
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

    match cli.command {
        Some(Command::Tui(tui)) => {
            if tui.once {
                println!("{}", render_once(120));
            } else if let Err(error) = run_app(cli.shared.no_alt_screen) {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        Some(Command::Repl) => {
            println!("Rust REPL bootstrap not wired yet.");
        }
        Some(Command::Chat { .. }) => {
            println!("Rust chat bootstrap not wired yet.");
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
