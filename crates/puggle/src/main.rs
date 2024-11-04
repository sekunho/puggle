pub use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
    #[arg(short, long)]
    pub config_path: Option<String>,
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
    color_eyre::install().unwrap();

    let cli = Args::parse();

    let config_path = match cli.config_path {
        Some(path) => path,
        None => "puggle.yaml".to_string(),
    };

    let config = puggle_lib::Config::from_file(config_path.as_str()).unwrap();

    match cli.command {
        Command::Server => puggle_server::run(config).await.unwrap(),
        Command::Build {} => puggle_lib::build_from_dir(config)
            .inspect_err(|e| println!("{:?}", e))
            .unwrap(),
    };
}
