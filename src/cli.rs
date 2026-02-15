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

    /// Enable verbose logging to ~/.local/state/ghw/debug.log
    #[arg(long)]
    pub verbose: bool,
}

/// Validates that `repo` matches the `owner/repo` pattern.
pub fn validate_repo_format(repo: &str) -> Result<(), String> {
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2
        || parts[0].is_empty()
        || parts[1].is_empty()
        || repo.contains(char::is_whitespace)
    {
        return Err(format!(
            "Invalid repo format '{repo}'. Expected 'owner/repo' (e.g. 'cli/cli')."
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_repo_format() {
        assert!(validate_repo_format("owner/repo").is_ok());
        assert!(validate_repo_format("cli/cli").is_ok());
        assert!(validate_repo_format("my-org/my-repo").is_ok());
    }

    #[test]
    fn invalid_repo_no_slash() {
        assert!(validate_repo_format("noslash").is_err());
    }

    #[test]
    fn invalid_repo_multiple_slashes() {
        assert!(validate_repo_format("a/b/c").is_err());
    }

    #[test]
    fn invalid_repo_empty_owner() {
        assert!(validate_repo_format("/repo").is_err());
    }

    #[test]
    fn invalid_repo_empty_name() {
        assert!(validate_repo_format("owner/").is_err());
    }

    #[test]
    fn invalid_repo_whitespace() {
        assert!(validate_repo_format("owner /repo").is_err());
    }

    #[test]
    fn invalid_repo_empty_string() {
        assert!(validate_repo_format("").is_err());
    }
}
