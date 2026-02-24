use async_trait::async_trait;
use ciw_core::traits::CiExecutor;
use color_eyre::eyre::{eyre, Result};
use std::time::Duration;
use tokio::process::Command;

const GLAB_TIMEOUT: Duration = Duration::from_secs(30);
const CLIPBOARD_TIMEOUT: Duration = Duration::from_secs(10);
const LOG_SIZE_LIMIT: usize = 10 * 1024 * 1024; // 10 MB

pub struct GlabExecutor {
    #[allow(dead_code)]
    project: String,
    encoded_project: String,
}

impl GlabExecutor {
    pub fn new(project: String) -> Self {
        let encoded_project = project.replace('/', "%2F");
        Self {
            project,
            encoded_project,
        }
    }
}

#[async_trait]
impl CiExecutor for GlabExecutor {
    async fn check_available(&self) -> Result<()> {
        run_glab(&["auth", "status"]).await.map(|_| ())
    }

    async fn detect_repo(&self) -> Result<String> {
        let output = run_glab(&["repo", "view", "--output", "json"]).await?;
        let parsed: serde_json::Value =
            serde_json::from_str(&output).map_err(|e| eyre!("Failed to parse repo info: {e}"))?;
        let project = parsed["path_with_namespace"]
            .as_str()
            .ok_or_else(|| eyre!("Could not detect project. Use --project flag."))?
            .to_string();
        if project.is_empty() {
            return Err(eyre!("Could not detect project. Use --project flag."));
        }
        Ok(project)
    }

    async fn detect_branch(&self) -> Result<String> {
        let output = tokio::time::timeout(
            GLAB_TIMEOUT,
            Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .output(),
        )
        .await
        .map_err(|_| eyre!("git command timed out after {}s", GLAB_TIMEOUT.as_secs()))?
        .map_err(|e| eyre!("Failed to detect branch: {}", e))?;

        if !output.status.success() {
            return Err(eyre!(
                "Failed to detect branch: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn fetch_runs(&self, limit: usize, source: Option<&str>) -> Result<String> {
        let mut url = format!(
            "/projects/{}/pipelines?per_page={}",
            self.encoded_project, limit
        );
        if let Some(s) = source {
            url.push_str(&format!("&source={}", s));
        }
        run_glab(&["api", &url]).await
    }

    async fn fetch_jobs(&self, pipeline_id: u64) -> Result<String> {
        let url = format!(
            "/projects/{}/pipelines/{}/jobs?per_page=100",
            self.encoded_project, pipeline_id
        );
        run_glab(&["api", &url]).await
    }

    async fn cancel_run(&self, pipeline_id: u64) -> Result<()> {
        let url = format!(
            "/projects/{}/pipelines/{}/cancel",
            self.encoded_project, pipeline_id
        );
        run_glab(&["api", "-X", "POST", &url]).await?;
        Ok(())
    }

    async fn delete_run(&self, pipeline_id: u64) -> Result<()> {
        let url = format!(
            "/projects/{}/pipelines/{}",
            self.encoded_project, pipeline_id
        );
        run_glab(&["api", "-X", "DELETE", &url]).await?;
        Ok(())
    }

    async fn rerun_failed(&self, pipeline_id: u64) -> Result<()> {
        let url = format!(
            "/projects/{}/pipelines/{}/retry",
            self.encoded_project, pipeline_id
        );
        run_glab(&["api", "-X", "POST", &url]).await?;
        Ok(())
    }

    /// Multi-step because GitLab has no `--log-failed` equivalent (unlike `gh`).
    /// Must: fetch all jobs → filter for failed status → fetch each job's trace endpoint.
    async fn fetch_failed_logs(&self, pipeline_id: u64) -> Result<String> {
        // 1. Fetch jobs for this pipeline
        let jobs_json = self.fetch_jobs(pipeline_id).await?;
        let jobs: serde_json::Value = serde_json::from_str(&jobs_json)
            .map_err(|e| eyre!("Failed to parse jobs response: {e}"))?;

        let jobs_array = jobs
            .as_array()
            .ok_or_else(|| eyre!("Expected jobs array from API"))?;

        // 2. Find failed jobs
        let failed_jobs: Vec<u64> = jobs_array
            .iter()
            .filter(|j| j["status"].as_str() == Some("failed"))
            .filter_map(|j| j["id"].as_u64())
            .collect();

        if failed_jobs.is_empty() {
            return Ok(String::new());
        }

        // 3. Fetch trace for each failed job
        let mut all_logs = String::new();
        for job_id in failed_jobs {
            let job_name = jobs_array
                .iter()
                .find(|j| j["id"].as_u64() == Some(job_id))
                .and_then(|j| j["name"].as_str())
                .unwrap_or("unknown");

            if !all_logs.is_empty() {
                all_logs.push_str("\n\n");
            }
            all_logs.push_str(&format!("=== Job: {} (id: {}) ===\n", job_name, job_id));

            match self.fetch_job_trace(job_id).await {
                Ok(trace) => all_logs.push_str(&trace),
                Err(e) => all_logs.push_str(&format!("(failed to fetch trace: {e})")),
            }
        }

        check_log_size(&all_logs)?;
        Ok(all_logs)
    }

    async fn fetch_failed_logs_for_job(&self, _pipeline_id: u64, job_id: u64) -> Result<String> {
        let trace = self.fetch_job_trace(job_id).await?;
        check_log_size(&trace)?;
        Ok(trace)
    }

    fn open_in_browser(&self, url: &str) -> Result<()> {
        open_in_browser_impl(url)
    }

    async fn copy_to_clipboard(&self, text: &str) -> Result<()> {
        copy_to_clipboard_impl(text).await
    }
}

impl GlabExecutor {
    async fn fetch_job_trace(&self, job_id: u64) -> Result<String> {
        let url = format!("/projects/{}/jobs/{}/trace", self.encoded_project, job_id);
        run_glab(&["api", &url]).await
    }
}

async fn run_glab(args: &[&str]) -> Result<String> {
    let start = std::time::Instant::now();
    let output = tokio::time::timeout(GLAB_TIMEOUT, Command::new("glab").args(args).output())
        .await
        .map_err(|_| eyre!("glab command timed out after {}s", GLAB_TIMEOUT.as_secs()))?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                eyre!("glab CLI not found. Install it from https://gitlab.com/gitlab-org/cli")
            } else {
                eyre!("Failed to run glab: {}", e)
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("{}", classify_glab_error(&stderr)));
    }

    tracing::debug!(
        args = ?args,
        elapsed_ms = start.elapsed().as_millis(),
        "glab command completed"
    );
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

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

pub fn classify_glab_error(stderr: &str) -> String {
    if stderr.contains("not logged") || stderr.contains("auth login") {
        "Not authenticated with glab. Run `glab auth login` first.".to_string()
    } else if stderr.contains("not a git repository") || stderr.contains("could not determine") {
        "Not in a GitLab repository. Use --project flag or cd into a repo.".to_string()
    } else {
        let trimmed = stderr.trim();
        if trimmed.is_empty() {
            "glab command failed".to_string()
        } else {
            format!("glab command failed: {trimmed}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_not_logged_in() {
        let msg = classify_glab_error("You are not logged into any GitLab hosts");
        assert!(msg.contains("Not authenticated"));
    }

    #[test]
    fn classify_auth_login() {
        let msg =
            classify_glab_error("To get started with GitLab CLI, please run: glab auth login");
        assert!(msg.contains("Not authenticated"));
    }

    #[test]
    fn classify_not_a_git_repo() {
        let msg = classify_glab_error("fatal: not a git repository (or any parent)");
        assert!(msg.contains("Not in a GitLab repository"));
    }

    #[test]
    fn classify_could_not_determine() {
        let msg = classify_glab_error("could not determine repo from current directory");
        assert!(msg.contains("Not in a GitLab repository"));
    }

    #[test]
    fn classify_generic_error() {
        let msg = classify_glab_error("something went wrong");
        assert_eq!(msg, "glab command failed: something went wrong");
    }

    #[test]
    fn classify_empty_stderr() {
        let msg = classify_glab_error("");
        assert_eq!(msg, "glab command failed");
    }

    #[test]
    fn classify_whitespace_only_stderr() {
        let msg = classify_glab_error("   \n  ");
        assert_eq!(msg, "glab command failed");
    }

    #[test]
    fn encoded_project_replaces_slashes() {
        let exec = GlabExecutor::new("group/subgroup/project".to_string());
        assert_eq!(exec.encoded_project, "group%2Fsubgroup%2Fproject");
    }

    #[test]
    fn encoded_project_simple() {
        let exec = GlabExecutor::new("mygroup/myproject".to_string());
        assert_eq!(exec.encoded_project, "mygroup%2Fmyproject");
    }
}
