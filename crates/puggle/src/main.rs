pub use clap::{Parser, Subcommand};

#[derive(Debug)]
pub struct Tree<T>
where
    T: PartialEq,
{
    pub arena: Vec<Node<T>>,
}

impl<T> Tree<T>
where
    T: PartialEq,
{
    pub fn new() -> Tree<T> {
        Tree { arena: Vec::new() }
    }
}

impl<T> Node<T>
where
    T: PartialEq,
{
    pub fn new_file(idx: i64, val: T) -> Node<T> {
        Node::File {
            idx,
            val,
            parent: None,
        }
    }
}

#[derive(Debug)]
pub enum Node<T>
where
    T: PartialEq,
{
    File {
        idx: i64,
        val: T,
        parent: Option<usize>,
    },
    Dir {
        idx: i64,
        val: T,
        parent: Option<usize>,
        children: Vec<usize>,
    },
}

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

    println!("{:#?}", config);

    match cli.command {
        Command::Server => puggle_server::run(&config).await.unwrap(),
        Command::Build {} => puggle_lib::build_from_dir(config)
            .inspect_err(|e| println!("{:?}", e))
            .unwrap(),
    };
}
