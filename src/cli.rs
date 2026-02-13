use clap::Parser;

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("BUILD_NUMBER"));

#[derive(Parser, Debug)]
#[command(name = "ghw", version = VERSION, about = "GitHub Actions Watcher TUI")]
pub struct Cli {
    /// Repository in owner/repo format (auto-detected from cwd)
    #[arg(short, long)]
    pub repo: Option<String>,

    /// Branch to filter (auto-detected from cwd)
    #[arg(short, long)]
    pub branch: Option<String>,

    /// Poll interval in seconds
    #[arg(short, long, default_value_t = 10)]
    pub interval: u64,

    /// Maximum number of runs to display
    #[arg(short, long, default_value_t = 20)]
    pub limit: usize,

    /// Filter to a specific workflow name
    #[arg(short, long)]
    pub workflow: Option<String>,

    /// Disable desktop notifications
    #[arg(long)]
    pub no_notify: bool,
}
