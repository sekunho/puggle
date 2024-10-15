pub use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Runs the server
    Server,
    /// Generates blog markdown files into full pages
    Build,
}

#[tokio::main]
async fn main() {
    let cli = Args::parse();
    color_eyre::install().unwrap();
    let config = puggle_lib::Config::from_file().unwrap();

    match cli.command {
        Command::Server => puggle_server::run(config).await.unwrap(),
        Command::Build {} => puggle_lib::build_from_dir(config)
            .inspect_err(|e| println!("{:?}", e))
            .unwrap(),
    };
}
