use async_trait::async_trait;
use ciw_core::traits::CiExecutor;
use color_eyre::eyre::{eyre, Result};
use std::time::Duration;
use tokio::process::Command;

const GH_TIMEOUT: Duration = Duration::from_secs(30);
const CLIPBOARD_TIMEOUT: Duration = Duration::from_secs(10);

pub struct GhExecutor {
    pub repo: String,
}

impl GhExecutor {
    pub fn new(repo: String) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl CiExecutor for GhExecutor {
    async fn check_available(&self) -> Result<()> {
        run_gh(&["auth", "status"]).await.map(|_| ())
    }

    async fn detect_repo(&self) -> Result<String> {
        let output = run_gh(&[
            "repo",
            "view",
            "--json",
            "nameWithOwner",
            "-q",
            ".nameWithOwner",
        ])
        .await?;
        let repo = output.trim().to_string();
        if repo.is_empty() {
            return Err(eyre!("Could not detect repository. Use --repo flag."));
        }
        Ok(repo)
    }

    async fn detect_branch(&self) -> Result<String> {
        let output = tokio::time::timeout(
            GH_TIMEOUT,
            Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .output(),
        )
        .await
        .map_err(|_| eyre!("git command timed out after {}s", GH_TIMEOUT.as_secs()))?
        .map_err(|e| eyre!("Failed to detect branch: {}", e))?;

        if !output.status.success() {
            return Err(eyre!(
                "Failed to detect branch: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn fetch_runs(&self, limit: usize, workflow: Option<&str>) -> Result<String> {
        let limit_str = limit.to_string();
        let mut args = vec![
            "run", "list",
            "--repo", &self.repo,
            "--limit", &limit_str,
            "--json", "databaseId,displayTitle,name,headBranch,status,conclusion,createdAt,updatedAt,event,number,url",
        ];
        if let Some(w) = workflow {
            args.push("--workflow");
            args.push(w);
        }
        run_gh(&args).await
    }

    async fn fetch_jobs(&self, run_id: u64) -> Result<String> {
        let run_id_str = run_id.to_string();
        run_gh(&[
            "run",
            "view",
            "--repo",
            &self.repo,
            &run_id_str,
            "--json",
            "jobs",
        ])
        .await
    }

    async fn cancel_run(&self, run_id: u64) -> Result<()> {
        let run_id_str = run_id.to_string();
        run_gh(&["run", "cancel", "--repo", &self.repo, &run_id_str]).await?;
        Ok(())
    }

    async fn delete_run(&self, run_id: u64) -> Result<()> {
        let run_id_str = run_id.to_string();
        run_gh(&["run", "delete", "--repo", &self.repo, &run_id_str]).await?;
        Ok(())
    }

    async fn rerun_failed(&self, run_id: u64) -> Result<()> {
        let run_id_str = run_id.to_string();
        run_gh(&[
            "run",
            "rerun",
            "--failed",
            "--repo",
            &self.repo,
            &run_id_str,
        ])
        .await?;
        Ok(())
    }

    async fn fetch_failed_logs(&self, run_id: u64) -> Result<String> {
        let run_id_str = run_id.to_string();
        let result = run_gh(&[
            "run",
            "view",
            "--repo",
            &self.repo,
            &run_id_str,
            "--log-failed",
        ])
        .await?;
        check_log_size(&result)?;
        Ok(result)
    }

    async fn fetch_failed_logs_for_job(&self, run_id: u64, job_id: u64) -> Result<String> {
        let run_id_str = run_id.to_string();
        let job_id_str = job_id.to_string();
        let result = run_gh(&[
            "run",
            "view",
            "--repo",
            &self.repo,
            &run_id_str,
            "--log-failed",
            "--job",
            &job_id_str,
        ])
        .await?;
        check_log_size(&result)?;
        Ok(result)
    }

    fn open_in_browser(&self, url: &str) -> Result<()> {
        open_in_browser_impl(url)
    }

    async fn copy_to_clipboard(&self, text: &str) -> Result<()> {
        copy_to_clipboard_impl(text).await
    }
}

async fn run_gh(args: &[&str]) -> Result<String> {
    let start = std::time::Instant::now();
    let output = tokio::time::timeout(GH_TIMEOUT, Command::new("gh").args(args).output())
        .await
        .map_err(|_| eyre!("gh command timed out after {}s", GH_TIMEOUT.as_secs()))?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                eyre!("gh CLI not found. Install it from https://cli.github.com/")
            } else {
                eyre!("Failed to run gh: {}", e)
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("{}", classify_gh_error(&stderr)));
    }

    tracing::debug!(
        args = ?args,
        elapsed_ms = start.elapsed().as_millis(),
        "gh command completed"
    );
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

const LOG_SIZE_LIMIT: usize = 10 * 1024 * 1024; // 10 MB

fn check_log_size(log: &str) -> Result<()> {
    if log.len() > LOG_SIZE_LIMIT {
        return Err(eyre!(
            "Log output too large ({:.1} MB, max {} MB)",
            log.len() as f64 / (1024.0 * 1024.0),
            LOG_SIZE_LIMIT / (1024 * 1024)
        ));
    }
    Ok(())
}

/// Opens a URL in the user's default browser.
///
/// Uses compile-time detection for Windows/macOS, then runtime detection for WSL2
/// (which compiles as `target_os = "linux"` but needs `wslview` instead of `xdg-open`).
fn open_in_browser_impl(url: &str) -> Result<()> {
    use std::process::{Command, Stdio};

    // Validate URL scheme to prevent opening arbitrary protocols or shell injection
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(eyre!("Refusing to open non-HTTP URL: {url}"));
    }

    if cfg!(target_os = "windows") {
        // Empty "" title parameter prevents the URL from being interpreted as a window title
        // and avoids shell metacharacter injection via cmd /C start
        return Command::new("cmd")
            .args(["/C", "start", "", url])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| eyre!("Failed to open browser: {e}"));
    }

    let cmds: &[&str] = if cfg!(target_os = "macos") {
        &["open"]
    } else if std::env::var_os("WSL_DISTRO_NAME").is_some() {
        &["wslview"]
    } else {
        &["xdg-open"]
    };

    for cmd in cmds {
        match Command::new(cmd)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(_) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(eyre!("Failed to open browser with {cmd}: {e}")),
        }
    }

    // WSL fallback: cmd.exe routes through Windows default browser
    if std::env::var_os("WSL_DISTRO_NAME").is_some() {
        return Command::new("cmd.exe")
            .args(["/C", "start", "", url])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| eyre!("Failed to open browser via cmd.exe: {e}"));
    }

    Err(eyre!(
        "No browser opener found. On WSL install wslu; on Linux install xdg-utils."
    ))
}

async fn copy_to_clipboard_impl(text: &str) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    // Determine clipboard command: try clip.exe first (WSL), then wl-copy (Wayland), then xclip (X11)
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else if cfg!(target_os = "windows") {
        &[("clip.exe", &[])]
    } else {
        // Linux: try WSL clip.exe first, then Wayland, then X11
        &[
            ("clip.exe", &[]),
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
        ]
    };

    for (cmd, args) in candidates {
        let child = tokio::process::Command::new(cmd)
            .args(*args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();

        if let Ok(mut child) = child {
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(text.as_bytes())
                    .await
                    .map_err(|e| eyre!("Failed to write to clipboard: {e}"))?;
                drop(stdin);
            }
            let status = tokio::time::timeout(CLIPBOARD_TIMEOUT, child.wait())
                .await
                .map_err(|_| {
                    eyre!(
                        "clipboard command timed out after {}s",
                        CLIPBOARD_TIMEOUT.as_secs()
                    )
                })??;
            if status.success() {
                return Ok(());
            }
        }
    }

    Err(eyre!(
        "No clipboard tool found. Install xclip, wl-copy, or use WSL with clip.exe"
    ))
}

pub fn classify_gh_error(stderr: &str) -> String {
    if stderr.contains("not logged") || stderr.contains("auth login") {
        "Not authenticated with gh. Run `gh auth login` first.".to_string()
    } else if stderr.contains("not a git repository") || stderr.contains("could not determine") {
        "Not in a GitHub repository. Use --repo flag or cd into a repo.".to_string()
    } else {
        let trimmed = stderr.trim();
        if trimmed.is_empty() {
            "gh command failed".to_string()
        } else {
            format!("gh command failed: {trimmed}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_not_logged_in() {
        let msg = classify_gh_error("You are not logged into any GitHub hosts");
        assert!(msg.contains("Not authenticated"));
    }

    #[test]
    fn classify_auth_login() {
        let msg = classify_gh_error("To get started with GitHub CLI, please run: gh auth login");
        assert!(msg.contains("Not authenticated"));
    }

    #[test]
    fn classify_not_a_git_repo() {
        let msg = classify_gh_error("fatal: not a git repository (or any parent)");
        assert!(msg.contains("Not in a GitHub repository"));
    }

    #[test]
    fn classify_could_not_determine() {
        let msg = classify_gh_error("could not determine repo from current directory");
        assert!(msg.contains("Not in a GitHub repository"));
    }

    #[test]
    fn classify_generic_error() {
        let msg = classify_gh_error("something went wrong");
        assert_eq!(msg, "gh command failed: something went wrong");
    }

    #[test]
    fn classify_empty_stderr() {
        let msg = classify_gh_error("");
        assert_eq!(msg, "gh command failed");
    }

    #[test]
    fn classify_whitespace_only_stderr() {
        let msg = classify_gh_error("   \n  ");
        assert_eq!(msg, "gh command failed");
    }
}
