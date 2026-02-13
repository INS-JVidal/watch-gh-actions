use color_eyre::eyre::{eyre, Result};
use tokio::process::Command;

pub async fn run_gh(args: &[&str]) -> Result<String> {
    let output = Command::new("gh").args(args).output().await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            eyre!("gh CLI not found. Install it from https://cli.github.com/")
        } else {
            eyre!("Failed to run gh: {}", e)
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not logged") || stderr.contains("auth login") {
            return Err(eyre!(
                "Not authenticated with gh. Run `gh auth login` first."
            ));
        }
        if stderr.contains("not a git repository") || stderr.contains("could not determine") {
            return Err(eyre!(
                "Not in a GitHub repository. Use --repo flag or cd into a repo."
            ));
        }
        return Err(eyre!("gh command failed: {}", stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub async fn check_gh_available() -> Result<()> {
    run_gh(&["auth", "status"]).await.map(|_| ())
}

pub async fn detect_repo() -> Result<String> {
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

pub async fn detect_branch() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .await
        .map_err(|e| eyre!("Failed to detect branch: {}", e))?;

    if !output.status.success() {
        return Err(eyre!("Failed to detect branch"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn fetch_runs(repo: &str, limit: usize, workflow: Option<&str>) -> Result<String> {
    let limit_str = limit.to_string();
    let mut args = vec![
        "run", "list",
        "--repo", repo,
        "--limit", &limit_str,
        "--json", "databaseId,displayTitle,name,headBranch,status,conclusion,createdAt,updatedAt,event,number,url",
    ];
    if let Some(w) = workflow {
        args.push("--workflow");
        args.push(w);
    }
    run_gh(&args).await
}

pub async fn fetch_jobs(repo: &str, run_id: u64) -> Result<String> {
    let run_id_str = run_id.to_string();
    run_gh(&["run", "view", "--repo", repo, &run_id_str, "--json", "jobs"]).await
}

pub async fn open_in_browser(url: &str) -> Result<()> {
    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", "start", url])
    } else {
        ("xdg-open", vec![url])
    };
    Command::new(cmd)
        .args(&args)
        .spawn()
        .map_err(|e| eyre!("Failed to open browser: {}", e))?;
    Ok(())
}

pub async fn rerun_failed(repo: &str, run_id: u64) -> Result<()> {
    let run_id_str = run_id.to_string();
    run_gh(&["run", "rerun", "--repo", repo, &run_id_str, "--failed"]).await?;
    Ok(())
}

pub async fn fetch_failed_logs(repo: &str, run_id: u64) -> Result<String> {
    let run_id_str = run_id.to_string();
    run_gh(&["run", "view", "--repo", repo, &run_id_str, "--log-failed"]).await
}

pub async fn fetch_failed_logs_for_job(repo: &str, run_id: u64, job_id: u64) -> Result<String> {
    let run_id_str = run_id.to_string();
    let job_id_str = job_id.to_string();
    run_gh(&[
        "run", "view", "--repo", repo, &run_id_str, "--log-failed", "--job", &job_id_str,
    ])
    .await
}

pub async fn copy_to_clipboard(text: &str) -> Result<()> {
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
                let _ = stdin.write_all(text.as_bytes()).await;
                drop(stdin);
            }
            let status = child.wait().await?;
            if status.success() {
                return Ok(());
            }
        }
    }

    Err(eyre!(
        "No clipboard tool found. Install xclip, wl-copy, or use WSL with clip.exe"
    ))
}
