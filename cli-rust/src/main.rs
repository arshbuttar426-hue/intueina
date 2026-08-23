use clap::{Parser, Subcommand};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_writer(std::io::stdout).init();
    let cli = Cli::parse();

    let cfg = intueina::config::AppConfig {
        port: cli.port,
        host: cli.host,
        embed_model: cli.embed_model,
        embed_enabled: !cli.no_embed,
        ..intueina::config::AppConfig::default()
    };

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => intueina::server::serve(cfg).await,
    }
}

#[derive(Parser)]
#[command(name = "intueina", version = "0.4.0", about = "Intueina exocortex (Rust)")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[arg(long, default_value = "9124")]
    port: u16,

    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value = "Snowflake/snowflake-arctic-embed-m")]
    embed_model: String,

    #[arg(long, default_value_t = false)]
    no_embed: bool,
}

#[derive(Subcommand)]
enum Command {
    Serve,
}
