use clap::Parser;

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("BUILD_NUMBER"));

#[derive(Parser, Debug)]
#[command(name = "glw", version = VERSION, about = "GitLab CI Watcher TUI")]
pub struct Cli {
    /// GitLab project path (e.g. group/project or group/subgroup/project)
    #[arg(short = 'p', long = "project")]
    pub project: Option<String>,

    /// Branch to filter (auto-detected from cwd)
    #[arg(short, long)]
    pub branch: Option<String>,

    /// Poll interval in seconds
    #[arg(short, long, default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..))]
    pub interval: u64,

    /// Maximum number of pipelines to display
    #[arg(short, long, default_value_t = 20)]
    pub limit: usize,

    /// Filter by pipeline source (push, schedule, merge_request_event, etc.)
    #[arg(short, long)]
    pub source: Option<String>,

    /// Disable desktop notifications
    #[arg(long)]
    pub no_notify: bool,

    /// Enable verbose logging to $XDG_STATE_HOME/glw/debug.log
    #[arg(long)]
    pub verbose: bool,
}

/// Validates that `project` has at least 2 segments (group/project).
pub fn validate_project_format(project: &str) -> Result<(), String> {
    let parts: Vec<&str> = project.split('/').collect();
    if parts.len() < 2
        || parts.iter().any(|p| p.is_empty())
        || project.contains(char::is_whitespace)
    {
        return Err(format!(
            "Invalid project format '{project}'. Expected 'group/project' (e.g. 'gitlab-org/gitlab')."
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_project_format() {
        assert!(validate_project_format("group/project").is_ok());
        assert!(validate_project_format("group/subgroup/project").is_ok());
        assert!(validate_project_format("a/b/c/d").is_ok());
    }

    #[test]
    fn invalid_project_no_slash() {
        assert!(validate_project_format("noslash").is_err());
    }

    #[test]
    fn invalid_project_empty_segment() {
        assert!(validate_project_format("/project").is_err());
        assert!(validate_project_format("group/").is_err());
        assert!(validate_project_format("a//b").is_err());
    }

    #[test]
    fn invalid_project_whitespace() {
        assert!(validate_project_format("group /project").is_err());
    }

    #[test]
    fn invalid_project_empty() {
        assert!(validate_project_format("").is_err());
    }
}
