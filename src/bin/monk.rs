use clap::Parser;
use monk_api_rust::cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    if let Err(e) = monk_api_rust::cli::run(cli).await {
        match std::env::var("CLI_VERBOSE").as_deref() {
            Ok("true") | Ok("1") => eprintln!("Error: {e:?}"),
            _ => eprintln!("Error: {e}"),
        }
        std::process::exit(1);
    }

    Ok(())
}