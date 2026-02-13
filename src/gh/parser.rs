use crate::app::{Job, WorkflowRun};
use color_eyre::eyre::Result;

pub fn parse_runs(json: &str) -> Result<Vec<WorkflowRun>> {
    let runs: Vec<WorkflowRun> = serde_json::from_str(json)?;
    Ok(runs)
}

#[derive(serde::Deserialize)]
struct JobsResponse {
    jobs: Vec<Job>,
}

pub fn parse_jobs(json: &str) -> Result<Vec<Job>> {
    let resp: JobsResponse = serde_json::from_str(json)?;
    Ok(resp.jobs)
}

/// Takes the last `max_lines` lines from raw log output.
/// Returns `(text, was_truncated)`.
pub fn process_log_output(raw: &str, max_lines: usize) -> (String, bool) {
    let lines: Vec<&str> = raw.lines().collect();
    if lines.len() > max_lines {
        let truncated = &lines[lines.len() - max_lines..];
        (truncated.join("\n"), true)
    } else {
        (raw.to_string(), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Conclusion, RunStatus};

    const SINGLE_RUN_JSON: &str = r#"[
        {
            "databaseId": 123,
            "displayTitle": "CI Build",
            "name": "tests",
            "headBranch": "main",
            "status": "completed",
            "conclusion": "success",
            "createdAt": "2024-01-15T10:00:00Z",
            "updatedAt": "2024-01-15T10:05:00Z",
            "event": "push",
            "number": 42,
            "url": "https://github.com/test/repo/actions/runs/123"
        }
    ]"#;

    #[test]
    fn parse_single_completed_run() {
        let runs = parse_runs(SINGLE_RUN_JSON).unwrap();
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        assert_eq!(run.database_id, 123);
        assert_eq!(run.display_title, "CI Build");
        assert_eq!(run.name, "tests");
        assert_eq!(run.head_branch, "main");
        assert_eq!(run.status, RunStatus::Completed);
        assert_eq!(run.conclusion, Some(Conclusion::Success));
        assert_eq!(run.event, "push");
        assert_eq!(run.number, 42);
        assert_eq!(run.url, "https://github.com/test/repo/actions/runs/123");
    }

    #[test]
    fn parse_in_progress_with_null_conclusion() {
        let json = r#"[{
            "databaseId": 1, "displayTitle": "t", "name": "n",
            "headBranch": "main", "status": "in_progress", "conclusion": null,
            "createdAt": "2024-01-01T00:00:00Z", "updatedAt": "2024-01-01T00:00:00Z",
            "event": "push", "number": 1, "url": "https://example.com"
        }]"#;
        let runs = parse_runs(json).unwrap();
        assert_eq!(runs[0].status, RunStatus::InProgress);
        assert_eq!(runs[0].conclusion, None);
    }

    #[test]
    fn parse_all_status_strings() {
        let statuses = [
            ("completed", RunStatus::Completed),
            ("in_progress", RunStatus::InProgress),
            ("queued", RunStatus::Queued),
            ("requested", RunStatus::Requested),
            ("waiting", RunStatus::Waiting),
            ("pending", RunStatus::Pending),
        ];
        for (s, expected) in &statuses {
            let json = format!(
                r#"[{{"databaseId":1,"displayTitle":"t","name":"n","headBranch":"m",
                "status":"{}","conclusion":null,
                "createdAt":"2024-01-01T00:00:00Z","updatedAt":"2024-01-01T00:00:00Z",
                "event":"push","number":1,"url":"u"}}]"#,
                s
            );
            let runs = parse_runs(&json).unwrap();
            assert_eq!(runs[0].status, *expected, "status string: {}", s);
        }
    }

    #[test]
    fn parse_unknown_status() {
        let json = r#"[{"databaseId":1,"displayTitle":"t","name":"n","headBranch":"m",
            "status":"something_new","conclusion":null,
            "createdAt":"2024-01-01T00:00:00Z","updatedAt":"2024-01-01T00:00:00Z",
            "event":"push","number":1,"url":"u"}]"#;
        let runs = parse_runs(json).unwrap();
        assert_eq!(runs[0].status, RunStatus::Unknown);
    }

    #[test]
    fn parse_all_conclusion_strings() {
        let conclusions = [
            ("success", Conclusion::Success),
            ("failure", Conclusion::Failure),
            ("cancelled", Conclusion::Cancelled),
            ("skipped", Conclusion::Skipped),
            ("timed_out", Conclusion::TimedOut),
            ("action_required", Conclusion::ActionRequired),
            ("startup_failure", Conclusion::StartupFailure),
            ("stale", Conclusion::Stale),
            ("neutral", Conclusion::Neutral),
        ];
        for (s, expected) in &conclusions {
            let json = format!(
                r#"[{{"databaseId":1,"displayTitle":"t","name":"n","headBranch":"m",
                "status":"completed","conclusion":"{}",
                "createdAt":"2024-01-01T00:00:00Z","updatedAt":"2024-01-01T00:00:00Z",
                "event":"push","number":1,"url":"u"}}]"#,
                s
            );
            let runs = parse_runs(&json).unwrap();
            assert_eq!(runs[0].conclusion, Some(*expected), "conclusion string: {}", s);
        }
    }

    #[test]
    fn parse_unknown_conclusion() {
        let json = r#"[{"databaseId":1,"displayTitle":"t","name":"n","headBranch":"m",
            "status":"completed","conclusion":"brand_new_thing",
            "createdAt":"2024-01-01T00:00:00Z","updatedAt":"2024-01-01T00:00:00Z",
            "event":"push","number":1,"url":"u"}]"#;
        let runs = parse_runs(json).unwrap();
        assert_eq!(runs[0].conclusion, Some(Conclusion::Unknown));
    }

    #[test]
    fn parse_empty_array() {
        let runs = parse_runs("[]").unwrap();
        assert!(runs.is_empty());
    }

    #[test]
    fn parse_multiple_runs() {
        let json = r#"[
            {"databaseId":1,"displayTitle":"A","name":"n","headBranch":"m",
             "status":"completed","conclusion":"success",
             "createdAt":"2024-01-01T00:00:00Z","updatedAt":"2024-01-01T00:00:00Z",
             "event":"push","number":1,"url":"u1"},
            {"databaseId":2,"displayTitle":"B","name":"n","headBranch":"m",
             "status":"in_progress","conclusion":null,
             "createdAt":"2024-01-01T00:00:00Z","updatedAt":"2024-01-01T00:00:00Z",
             "event":"push","number":2,"url":"u2"}
        ]"#;
        let runs = parse_runs(json).unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].display_title, "A");
        assert_eq!(runs[1].display_title, "B");
    }

    #[test]
    fn parse_invalid_json_error() {
        assert!(parse_runs("not json").is_err());
    }

    #[test]
    fn parse_missing_fields_error() {
        let json = r#"[{"databaseId": 1}]"#;
        assert!(parse_runs(json).is_err());
    }

    #[test]
    fn parse_unicode_title() {
        let json = r#"[{"databaseId":1,"displayTitle":"构建 🚀 テスト","name":"n","headBranch":"m",
            "status":"completed","conclusion":"success",
            "createdAt":"2024-01-01T00:00:00Z","updatedAt":"2024-01-01T00:00:00Z",
            "event":"push","number":1,"url":"u"}]"#;
        let runs = parse_runs(json).unwrap();
        assert_eq!(runs[0].display_title, "构建 🚀 テスト");
    }

    #[test]
    fn parse_jobs_with_steps() {
        let json = r#"{"jobs":[
            {
                "name": "build",
                "status": "completed",
                "conclusion": "success",
                "startedAt": "2024-01-01T00:00:00Z",
                "completedAt": "2024-01-01T00:05:00Z",
                "url": "https://example.com/job/1",
                "steps": [
                    {"name": "Checkout", "status": "completed", "conclusion": "success", "number": 1},
                    {"name": "Build", "status": "completed", "conclusion": "failure", "number": 2}
                ]
            }
        ]}"#;
        let jobs = parse_jobs(json).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].name, "build");
        assert_eq!(jobs[0].conclusion, Some(Conclusion::Success));
        assert_eq!(jobs[0].steps.len(), 2);
        assert_eq!(jobs[0].steps[0].name, "Checkout");
        assert_eq!(jobs[0].steps[1].conclusion, Some(Conclusion::Failure));
    }

    #[test]
    fn parse_jobs_empty() {
        let json = r#"{"jobs":[]}"#;
        let jobs = parse_jobs(json).unwrap();
        assert!(jobs.is_empty());
    }

    #[test]
    fn parse_jobs_null_timestamps() {
        let json = r#"{"jobs":[{
            "name": "build", "status": "queued", "conclusion": null,
            "startedAt": null, "completedAt": null,
            "url": "https://example.com", "steps": []
        }]}"#;
        let jobs = parse_jobs(json).unwrap();
        assert!(jobs[0].started_at.is_none());
        assert!(jobs[0].completed_at.is_none());
    }

    #[test]
    fn parse_jobs_invalid_wrapper_error() {
        assert!(parse_jobs(r#"{"not_jobs": []}"#).is_err());
    }

    #[test]
    fn process_log_output_no_truncation() {
        let raw = "line 1\nline 2\nline 3";
        let (text, truncated) = process_log_output(raw, 10);
        assert_eq!(text, raw);
        assert!(!truncated);
    }

    #[test]
    fn process_log_output_truncates() {
        let raw = (0..20).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        let (text, truncated) = process_log_output(&raw, 5);
        assert!(truncated);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "line 15");
        assert_eq!(lines[4], "line 19");
    }

    #[test]
    fn process_log_output_exact_limit() {
        let raw = "a\nb\nc";
        let (text, truncated) = process_log_output(raw, 3);
        assert_eq!(text, raw);
        assert!(!truncated);
    }

    #[test]
    fn process_log_output_empty() {
        let (text, truncated) = process_log_output("", 10);
        assert_eq!(text, "");
        assert!(!truncated);
    }
}
