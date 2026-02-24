//! Animated multi-phase startup screen.
//!
//! Each phase (check CLI, detect repo, detect branch, fetch runs) is driven by
//! `tokio::select!` to run the async future concurrently with an 80ms spinner tick,
//! re-rendering the startup screen on each tick so the animation stays smooth even
//! during slow network calls.

use crate::app::WorkflowRun;
use crate::platform::PlatformConfig;
use crate::traits::{CiExecutor, CiParser};
use crate::tui::spinner;
use color_eyre::eyre::{eyre, Result};
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;
use std::future::Future;
use std::time::Duration;

/// Interpolate a 3-stop gradient: Red -> Magenta -> Violet across `total_lines`.
fn gradient_color(line_idx: usize, total_lines: usize) -> Color {
    if total_lines <= 1 {
        return Color::Rgb(255, 0, 0);
    }
    let t = line_idx as f64 / (total_lines - 1) as f64;

    let (r, g, b) = if t <= 0.5 {
        // Red (255, 0, 0) -> Magenta (220, 0, 155)
        let s = t * 2.0;
        ((255.0 + (220.0 - 255.0) * s) as u8, 0, (155.0 * s) as u8)
    } else {
        // Magenta (220, 0, 155) -> Violet (136, 0, 255)
        let s = (t - 0.5) * 2.0;
        (
            (220.0 + (136.0 - 220.0) * s) as u8,
            0,
            (155.0 + (255.0 - 155.0) * s) as u8,
        )
    };

    Color::Rgb(r, g, b)
}

#[derive(Clone)]
enum PhaseStatus {
    InProgress,
    Done,
    Failed(String),
}

#[derive(Clone)]
struct StartupPhase {
    label: String,
    detail: Option<String>,
    status: PhaseStatus,
}

pub struct StartupResult {
    pub repo: String,
    pub branch: Option<String>,
    pub runs: Vec<WorkflowRun>,
}

fn render_startup<B: Backend>(
    terminal: &mut Terminal<B>,
    ascii_art: &[&str],
    phases: &[StartupPhase],
    frame: usize,
) {
    if let Err(e) = terminal.draw(|f| {
        let area = f.area();
        let art_height = ascii_art.len() as u16;
        let total_lines = art_height + 1 + phases.len() as u16;
        let top_offset = (area.height.saturating_sub(total_lines) / 2).saturating_sub(4);
        let vertical = Layout::vertical([
            Constraint::Length(top_offset),
            Constraint::Length(total_lines),
            Constraint::Min(0),
        ])
        .split(area);

        let mut lines: Vec<Line> = ascii_art
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let color = gradient_color(i, ascii_art.len());
                Line::from(Span::styled(*line, Style::default().fg(color)))
            })
            .collect();

        lines.push(Line::from(""));

        lines.extend(phases.iter().map(|phase| {
            let (icon, icon_style) = match &phase.status {
                PhaseStatus::InProgress => (
                    spinner::frame(frame).to_string(),
                    Style::default().fg(Color::Yellow),
                ),
                PhaseStatus::Done => ("\u{2713}".to_string(), Style::default().fg(Color::Green)),
                PhaseStatus::Failed(_) => ("\u{2717}".to_string(), Style::default().fg(Color::Red)),
            };

            let mut spans = vec![
                Span::styled(format!("  {icon} "), icon_style),
                Span::styled(&phase.label, Style::default().fg(Color::White)),
            ];

            if let Some(detail) = &phase.detail {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(detail, Style::default().fg(Color::DarkGray)));
            }

            if let PhaseStatus::Failed(msg) = &phase.status {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(msg, Style::default().fg(Color::Red)));
            }

            Line::from(spans)
        }));

        let paragraph = Paragraph::new(lines);
        f.render_widget(paragraph, vertical[1]);
    }) {
        tracing::warn!("startup render failed: {e}");
    }
}

async fn run_phase<B, F, T>(
    terminal: &mut Terminal<B>,
    ascii_art: &[&str],
    phases: &mut Vec<StartupPhase>,
    label: &str,
    fut: F,
) -> Result<T>
where
    B: Backend,
    F: Future<Output = Result<T>>,
{
    phases.push(StartupPhase {
        label: label.to_string(),
        detail: None,
        status: PhaseStatus::InProgress,
    });
    render_startup(terminal, ascii_art, phases, 0);

    let mut ticker = tokio::time::interval(Duration::from_millis(80));
    let mut frame = 0usize;
    tokio::pin!(fut);

    loop {
        tokio::select! {
            result = &mut fut => {
                let idx = phases.len() - 1;
                match &result {
                    Ok(_) => phases[idx].status = PhaseStatus::Done,
                    Err(e) => phases[idx].status = PhaseStatus::Failed(e.to_string()),
                }
                render_startup(terminal, ascii_art, phases, frame);
                return result;
            }
            _ = ticker.tick() => {
                frame += 1;
                render_startup(terminal, ascii_art, phases, frame);
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub async fn run_startup<B: Backend>(
    terminal: &mut Terminal<B>,
    platform: &PlatformConfig,
    executor: &dyn CiExecutor,
    parser: &dyn CiParser,
    repo_arg: Option<&str>,
    branch_arg: Option<&str>,
    limit: usize,
    filter: Option<&str>,
    validate_repo: Option<fn(&str) -> Result<(), String>>,
) -> Result<StartupResult> {
    let mut phases: Vec<StartupPhase> = Vec::new();
    let art = platform.ascii_art;

    // Phase 1: Check CI CLI
    run_phase(
        terminal,
        art,
        &mut phases,
        &format!("Checking {} CLI", platform.cli_tool),
        executor.check_available(),
    )
    .await?;

    // Phase 2: Detect repository
    let repo = if let Some(r) = repo_arg {
        let repo = r.to_string();
        phases.push(StartupPhase {
            label: "Detecting repository".to_string(),
            detail: Some(repo.clone()),
            status: PhaseStatus::Done,
        });
        render_startup(terminal, art, &phases, 0);
        repo
    } else {
        let repo = run_phase(
            terminal,
            art,
            &mut phases,
            "Detecting repository",
            executor.detect_repo(),
        )
        .await?;
        let idx = phases.len() - 1;
        phases[idx].detail = Some(repo.clone());
        render_startup(terminal, art, &phases, 0);
        repo
    };

    // Validate repo format
    if let Some(validate) = validate_repo {
        validate(&repo).map_err(|e| eyre!("{e}"))?;
    }

    // Phase 3: Detect branch (non-fatal)
    let branch = if let Some(b) = branch_arg {
        let branch = b.to_string();
        phases.push(StartupPhase {
            label: "Detecting branch".to_string(),
            detail: Some(branch.clone()),
            status: PhaseStatus::Done,
        });
        render_startup(terminal, art, &phases, 0);
        Some(branch)
    } else {
        let result = run_phase(
            terminal,
            art,
            &mut phases,
            "Detecting branch",
            executor.detect_branch(),
        )
        .await;
        if let Ok(b) = result {
            let idx = phases.len() - 1;
            phases[idx].detail = Some(b.clone());
            render_startup(terminal, art, &phases, 0);
            Some(b)
        } else {
            // Non-fatal: mark as done but indicate it was skipped
            let idx = phases.len() - 1;
            phases[idx].status = PhaseStatus::Done;
            phases[idx].detail = Some("(skipped)".to_string());
            render_startup(terminal, art, &phases, 0);
            None
        }
    };

    // Phase 4: Fetch workflow runs
    let json = run_phase(
        terminal,
        art,
        &mut phases,
        &format!("Fetching {} runs", platform.name),
        executor.fetch_runs(limit, filter),
    )
    .await?;

    let runs = parser.parse_runs(&json)?;
    let idx = phases.len() - 1;
    phases[idx].detail = Some(format!("{} runs", runs.len()));
    render_startup(terminal, art, &phases, 0);

    Ok(StartupResult { repo, branch, runs })
}
