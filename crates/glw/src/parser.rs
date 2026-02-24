use chrono::{DateTime, Utc};
use ciw_core::app::{Conclusion, Job, RunStatus, WorkflowRun};
use ciw_core::traits::CiParser;
use color_eyre::eyre::{eyre, Result};
use serde::Deserialize;

const MAX_RESPONSE_SIZE: usize = 10 * 1024 * 1024; // 10 MB

fn check_response_size(json: &str) -> Result<()> {
    if json.len() > MAX_RESPONSE_SIZE {
        return Err(eyre!(
            "Response too large ({:.1} MB, max {} MB)",
            json.len() as f64 / (1024.0 * 1024.0),
            MAX_RESPONSE_SIZE / (1024 * 1024)
        ));
    }
    Ok(())
}

// -- Intermediate GitLab pipeline struct --

#[derive(Deserialize, Debug)]
struct GlabPipeline {
    id: u64,
    #[serde(default)]
    iid: u64,
    #[serde(rename = "ref")]
    git_ref: String,
    status: String,
    #[serde(default)]
    source: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    web_url: String,
}

// -- Intermediate GitLab job struct --

#[derive(Deserialize, Debug)]
struct GlabJob {
    id: u64,
    name: String,
    status: String,
    #[serde(default)]
    started_at: Option<String>,
    #[serde(default)]
    finished_at: Option<String>,
    #[serde(default)]
    web_url: String,
}

/// GitLab uses a single `status` field where GitHub uses `status` + `conclusion`.
/// These two functions split GitLab's flat status into the two-dimensional model:
/// `map_status` determines if the run is finished/active/pending, while
/// `map_conclusion` extracts the outcome (only meaningful for terminal statuses).
fn map_status(status: &str) -> RunStatus {
    match status {
        "success" | "failed" | "canceled" | "skipped" => RunStatus::Completed,
        "running" => RunStatus::InProgress,
        "pending" | "created" | "preparing" | "manual" | "scheduled" => RunStatus::Pending,
        "waiting_for_resource" => RunStatus::Waiting,
        _ => RunStatus::Unknown,
    }
}

fn map_conclusion(status: &str) -> Option<Conclusion> {
    match status {
        "success" => Some(Conclusion::Success),
        "failed" => Some(Conclusion::Failure),
        "canceled" => Some(Conclusion::Cancelled),
        "skipped" => Some(Conclusion::Skipped),
        // Non-terminal statuses have no conclusion
        "running"
        | "pending"
        | "created"
        | "waiting_for_resource"
        | "preparing"
        | "manual"
        | "scheduled" => None,
        _ => None,
    }
}

fn parse_datetime(s: &str) -> Result<DateTime<Utc>> {
    // GitLab returns ISO 8601 timestamps, possibly with timezone offset
    s.parse::<DateTime<Utc>>()
        .or_else(|_| {
            // Try parsing with chrono's flexible parser for formats like "2024-01-15T10:00:00.000+00:00"
            DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc))
        })
        .map_err(|e| eyre!("Failed to parse datetime '{}': {}", s, e))
}

fn parse_optional_datetime(s: &Option<String>) -> Option<DateTime<Utc>> {
    s.as_ref().and_then(|s| parse_datetime(s).ok())
}

impl From<GlabPipeline> for WorkflowRun {
    fn from(p: GlabPipeline) -> Self {
        let status = map_status(&p.status);
        let conclusion = map_conclusion(&p.status);
        let created_at = parse_datetime(&p.created_at).unwrap_or_else(|_| Utc::now());
        let updated_at = parse_datetime(&p.updated_at).unwrap_or_else(|_| Utc::now());

        WorkflowRun {
            database_id: p.id,
            number: p.iid,
            display_title: format!("Pipeline #{}", p.iid),
            name: p.source.clone(),
            head_branch: p.git_ref,
            status,
            conclusion,
            created_at,
            updated_at,
            event: p.source,
            url: p.web_url,
            jobs: None,
        }
    }
}

impl From<GlabJob> for Job {
    fn from(j: GlabJob) -> Self {
        let status = map_status(&j.status);
        let conclusion = map_conclusion(&j.status);

        Job {
            database_id: Some(j.id),
            name: j.name,
            status,
            conclusion,
            started_at: parse_optional_datetime(&j.started_at),
            completed_at: parse_optional_datetime(&j.finished_at),
            url: j.web_url,
            steps: vec![],
        }
    }
}

pub struct GlabParser;

impl CiParser for GlabParser {
    fn parse_runs(&self, json: &str) -> Result<Vec<WorkflowRun>> {
        check_response_size(json)?;
        let pipelines: Vec<GlabPipeline> = serde_json::from_str(json)?;
        Ok(pipelines.into_iter().map(WorkflowRun::from).collect())
    }

    fn parse_jobs(&self, json: &str) -> Result<Vec<Job>> {
        check_response_size(json)?;
        let jobs: Vec<GlabJob> = serde_json::from_str(json)?;
        Ok(jobs.into_iter().map(Job::from).collect())
    }

    fn process_log_output(&self, raw: &str, max_lines: usize) -> (String, bool) {
        let lines: Vec<&str> = raw.lines().collect();
        if lines.len() > max_lines {
            let truncated = &lines[lines.len() - max_lines..];
            (truncated.join("\n"), true)
        } else {
            (raw.to_string(), false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser() -> GlabParser {
        GlabParser
    }

    const SINGLE_PIPELINE_JSON: &str = r#"[
        {
            "id": 123456,
            "iid": 42,
            "ref": "main",
            "status": "success",
            "source": "push",
            "created_at": "2024-01-15T10:00:00Z",
            "updated_at": "2024-01-15T10:05:00Z",
            "web_url": "https://gitlab.com/group/project/-/pipelines/123456"
        }
    ]"#;

    #[test]
    fn parse_single_completed_pipeline() {
        let p = parser();
        let runs = p.parse_runs(SINGLE_PIPELINE_JSON).unwrap();
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        assert_eq!(run.database_id, 123456);
        assert_eq!(run.number, 42);
        assert_eq!(run.display_title, "Pipeline #42");
        assert_eq!(run.name, "push");
        assert_eq!(run.head_branch, "main");
        assert_eq!(run.status, RunStatus::Completed);
        assert_eq!(run.conclusion, Some(Conclusion::Success));
        assert_eq!(run.event, "push");
        assert_eq!(
            run.url,
            "https://gitlab.com/group/project/-/pipelines/123456"
        );
    }

    #[test]
    fn parse_running_pipeline() {
        let json = r#"[{
            "id": 1, "iid": 1, "ref": "main", "status": "running",
            "source": "push",
            "created_at": "2024-01-01T00:00:00Z", "updated_at": "2024-01-01T00:00:00Z",
            "web_url": "https://example.com"
        }]"#;
        let p = parser();
        let runs = p.parse_runs(json).unwrap();
        assert_eq!(runs[0].status, RunStatus::InProgress);
        assert_eq!(runs[0].conclusion, None);
    }

    #[test]
    fn parse_all_status_mappings() {
        let p = parser();
        let cases = [
            ("success", RunStatus::Completed, Some(Conclusion::Success)),
            ("failed", RunStatus::Completed, Some(Conclusion::Failure)),
            (
                "canceled",
                RunStatus::Completed,
                Some(Conclusion::Cancelled),
            ),
            ("skipped", RunStatus::Completed, Some(Conclusion::Skipped)),
            ("running", RunStatus::InProgress, None),
            ("pending", RunStatus::Pending, None),
            ("created", RunStatus::Pending, None),
            ("waiting_for_resource", RunStatus::Waiting, None),
            ("preparing", RunStatus::Pending, None),
            ("manual", RunStatus::Pending, None),
            ("scheduled", RunStatus::Pending, None),
        ];
        for (status_str, expected_status, expected_conclusion) in &cases {
            let json = format!(
                r#"[{{"id":1,"iid":1,"ref":"m","status":"{}","source":"push",
                "created_at":"2024-01-01T00:00:00Z","updated_at":"2024-01-01T00:00:00Z",
                "web_url":"u"}}]"#,
                status_str
            );
            let runs = p.parse_runs(&json).unwrap();
            assert_eq!(
                runs[0].status, *expected_status,
                "status mismatch for '{}'",
                status_str
            );
            assert_eq!(
                runs[0].conclusion, *expected_conclusion,
                "conclusion mismatch for '{}'",
                status_str
            );
        }
    }

    #[test]
    fn parse_unknown_status() {
        let json = r#"[{"id":1,"iid":1,"ref":"m","status":"something_new","source":"push",
            "created_at":"2024-01-01T00:00:00Z","updated_at":"2024-01-01T00:00:00Z",
            "web_url":"u"}]"#;
        let p = parser();
        let runs = p.parse_runs(json).unwrap();
        assert_eq!(runs[0].status, RunStatus::Unknown);
        assert_eq!(runs[0].conclusion, None);
    }

    #[test]
    fn parse_empty_array() {
        let p = parser();
        let runs = p.parse_runs("[]").unwrap();
        assert!(runs.is_empty());
    }

    #[test]
    fn parse_multiple_pipelines() {
        let json = r#"[
            {"id":1,"iid":1,"ref":"main","status":"success","source":"push",
             "created_at":"2024-01-01T00:00:00Z","updated_at":"2024-01-01T00:00:00Z","web_url":"u1"},
            {"id":2,"iid":2,"ref":"main","status":"running","source":"push",
             "created_at":"2024-01-01T00:00:00Z","updated_at":"2024-01-01T00:00:00Z","web_url":"u2"}
        ]"#;
        let p = parser();
        let runs = p.parse_runs(json).unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].display_title, "Pipeline #1");
        assert_eq!(runs[1].display_title, "Pipeline #2");
    }

    #[test]
    fn parse_invalid_json_error() {
        let p = parser();
        assert!(p.parse_runs("not json").is_err());
    }

    #[test]
    fn parse_missing_fields_error() {
        let json = r#"[{"id": 1}]"#;
        let p = parser();
        assert!(p.parse_runs(json).is_err());
    }

    #[test]
    fn parse_jobs_basic() {
        let json = r#"[
            {
                "id": 100,
                "name": "build",
                "status": "success",
                "started_at": "2024-01-01T00:00:00Z",
                "finished_at": "2024-01-01T00:05:00Z",
                "web_url": "https://gitlab.com/group/project/-/jobs/100"
            },
            {
                "id": 101,
                "name": "test",
                "status": "failed",
                "started_at": "2024-01-01T00:05:00Z",
                "finished_at": "2024-01-01T00:10:00Z",
                "web_url": "https://gitlab.com/group/project/-/jobs/101"
            }
        ]"#;
        let p = parser();
        let jobs = p.parse_jobs(json).unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].name, "build");
        assert_eq!(jobs[0].database_id, Some(100));
        assert_eq!(jobs[0].status, RunStatus::Completed);
        assert_eq!(jobs[0].conclusion, Some(Conclusion::Success));
        assert!(jobs[0].started_at.is_some());
        assert!(jobs[0].completed_at.is_some());
        assert!(jobs[0].steps.is_empty());
        assert_eq!(jobs[1].name, "test");
        assert_eq!(jobs[1].conclusion, Some(Conclusion::Failure));
    }

    #[test]
    fn parse_jobs_empty() {
        let json = "[]";
        let p = parser();
        let jobs = p.parse_jobs(json).unwrap();
        assert!(jobs.is_empty());
    }

    #[test]
    fn parse_jobs_null_timestamps() {
        let json = r#"[{
            "id": 1, "name": "build", "status": "pending",
            "started_at": null, "finished_at": null,
            "web_url": "https://example.com"
        }]"#;
        let p = parser();
        let jobs = p.parse_jobs(json).unwrap();
        assert!(jobs[0].started_at.is_none());
        assert!(jobs[0].completed_at.is_none());
        assert_eq!(jobs[0].conclusion, None);
    }

    #[test]
    fn parse_jobs_missing_timestamps() {
        let json = r#"[{
            "id": 1, "name": "build", "status": "created",
            "web_url": "https://example.com"
        }]"#;
        let p = parser();
        let jobs = p.parse_jobs(json).unwrap();
        assert!(jobs[0].started_at.is_none());
        assert!(jobs[0].completed_at.is_none());
    }

    #[test]
    fn process_log_output_no_truncation() {
        let p = parser();
        let raw = "line 1\nline 2\nline 3";
        let (text, truncated) = p.process_log_output(raw, 10);
        assert_eq!(text, raw);
        assert!(!truncated);
    }

    #[test]
    fn process_log_output_truncates() {
        let p = parser();
        let raw = (0..20)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let (text, truncated) = p.process_log_output(&raw, 5);
        assert!(truncated);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "line 15");
        assert_eq!(lines[4], "line 19");
    }

    #[test]
    fn process_log_output_exact_limit() {
        let p = parser();
        let raw = "a\nb\nc";
        let (text, truncated) = p.process_log_output(raw, 3);
        assert_eq!(text, raw);
        assert!(!truncated);
    }

    #[test]
    fn process_log_output_empty() {
        let p = parser();
        let (text, truncated) = p.process_log_output("", 10);
        assert_eq!(text, "");
        assert!(!truncated);
    }

    #[test]
    fn parse_runs_rejects_oversized_response() {
        let p = parser();
        let huge = "x".repeat(11 * 1024 * 1024);
        let err = p.parse_runs(&huge).unwrap_err();
        assert!(err.to_string().contains("too large"));
    }

    #[test]
    fn parse_jobs_rejects_oversized_response() {
        let p = parser();
        let huge = "x".repeat(11 * 1024 * 1024);
        let err = p.parse_jobs(&huge).unwrap_err();
        assert!(err.to_string().contains("too large"));
    }

    #[test]
    fn parse_runs_accepts_within_limit() {
        let p = parser();
        assert!(p.parse_runs("[]").is_ok());
    }

    #[test]
    fn parse_pipeline_with_rfc3339_offset() {
        let json = r#"[{
            "id": 1, "iid": 1, "ref": "main", "status": "success",
            "source": "push",
            "created_at": "2024-01-15T10:00:00.000+02:00",
            "updated_at": "2024-01-15T12:00:00.000+02:00",
            "web_url": "u"
        }]"#;
        let p = parser();
        let runs = p.parse_runs(json).unwrap();
        assert_eq!(runs.len(), 1);
    }

    #[test]
    fn display_title_format() {
        let json = r#"[{
            "id": 999, "iid": 77, "ref": "main", "status": "success",
            "source": "merge_request_event",
            "created_at": "2024-01-01T00:00:00Z", "updated_at": "2024-01-01T00:00:00Z",
            "web_url": "u"
        }]"#;
        let p = parser();
        let runs = p.parse_runs(json).unwrap();
        assert_eq!(runs[0].display_title, "Pipeline #77");
        assert_eq!(runs[0].name, "merge_request_event");
        assert_eq!(runs[0].event, "merge_request_event");
    }
}
