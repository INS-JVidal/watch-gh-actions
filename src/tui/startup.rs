use crate::app::WorkflowRun;
use crate::cli::Cli;
use crate::gh;
use crate::tui::spinner;
use color_eyre::eyre::Result;
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;
use std::future::Future;
use std::time::Duration;

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
    phases: &[StartupPhase],
    frame: usize,
) {
    let _ = terminal.draw(|f| {
        let area = f.area();
        let total_lines = phases.len() as u16;
        let vertical = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(total_lines),
            Constraint::Min(0),
        ])
        .split(area);

        let lines: Vec<Line> = phases
            .iter()
            .map(|phase| {
                let (icon, icon_style) = match &phase.status {
                    PhaseStatus::InProgress => (
                        spinner::frame(frame).to_string(),
                        Style::default().fg(Color::Yellow),
                    ),
                    PhaseStatus::Done => {
                        ("\u{2713}".to_string(), Style::default().fg(Color::Green))
                    }
                    PhaseStatus::Failed(_) => {
                        ("\u{2717}".to_string(), Style::default().fg(Color::Red))
                    }
                };

                let mut spans = vec![
                    Span::styled(format!("  {} ", icon), icon_style),
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
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        f.render_widget(paragraph, vertical[1]);
    });
}

async fn run_phase<B, F, T>(
    terminal: &mut Terminal<B>,
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
    render_startup(terminal, phases, 0);

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
                render_startup(terminal, phases, frame);
                return result;
            }
            _ = ticker.tick() => {
                frame += 1;
                render_startup(terminal, phases, frame);
            }
        }
    }
}

pub async fn run_startup<B: Backend>(
    terminal: &mut Terminal<B>,
    args: &Cli,
) -> Result<StartupResult> {
    let mut phases: Vec<StartupPhase> = Vec::new();

    // Phase 1: Check GitHub CLI
    run_phase(
        terminal,
        &mut phases,
        "Checking GitHub CLI",
        gh::executor::check_gh_available(),
    )
    .await?;

    // Phase 2: Detect repository
    let repo = if let Some(ref r) = args.repo {
        let repo = r.clone();
        phases.push(StartupPhase {
            label: "Detecting repository".to_string(),
            detail: Some(repo.clone()),
            status: PhaseStatus::Done,
        });
        render_startup(terminal, &phases, 0);
        repo
    } else {
        let repo = run_phase(
            terminal,
            &mut phases,
            "Detecting repository",
            gh::executor::detect_repo(),
        )
        .await?;
        let idx = phases.len() - 1;
        phases[idx].detail = Some(repo.clone());
        render_startup(terminal, &phases, 0);
        repo
    };

    // Phase 3: Detect branch (non-fatal)
    let branch = if let Some(ref b) = args.branch {
        let branch = b.clone();
        phases.push(StartupPhase {
            label: "Detecting branch".to_string(),
            detail: Some(branch.clone()),
            status: PhaseStatus::Done,
        });
        render_startup(terminal, &phases, 0);
        Some(branch)
    } else {
        let result = run_phase(
            terminal,
            &mut phases,
            "Detecting branch",
            gh::executor::detect_branch(),
        )
        .await;
        match result {
            Ok(b) => {
                let idx = phases.len() - 1;
                phases[idx].detail = Some(b.clone());
                render_startup(terminal, &phases, 0);
                Some(b)
            }
            Err(_) => {
                // Non-fatal: mark as done with no detail
                let idx = phases.len() - 1;
                phases[idx].status = PhaseStatus::Done;
                render_startup(terminal, &phases, 0);
                None
            }
        }
    };

    // Phase 4: Fetch workflow runs
    let json = run_phase(
        terminal,
        &mut phases,
        "Fetching workflow runs",
        gh::executor::fetch_runs(&repo, args.limit, args.workflow.as_deref()),
    )
    .await?;

    let runs = gh::parser::parse_runs(&json)?;
    let idx = phases.len() - 1;
    phases[idx].detail = Some(format!("{} runs", runs.len()));
    render_startup(terminal, &phases, 0);

    Ok(StartupResult { repo, branch, runs })
}
