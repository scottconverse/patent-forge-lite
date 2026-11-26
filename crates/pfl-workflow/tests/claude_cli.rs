//! The Claude CLI transport, against fake programs and the real output shape.
//!
//! The fixture in `parses_the_real_envelope` is a captured line from this machine's CLI, warning
//! line and all — the CLI prints an advisor notice to stdout before its JSON, and a parser that
//! expects pure JSON fails on it.
use pfl_domain::*;
use pfl_workflow::cli::{ClaudeCliProvider, parse_claude_result, redact};
use std::time::Duration;
use tempfile::tempdir;

const REAL_ENVELOPE: &str = concat!(
    "Warning: Advisor disabled — base model 'claude-sonnet-4-5' has no advisor rank in the model catalog.\n",
    "{\"duration_api_ms\":3342,\"stop_reason\":\"end_turn\",\"total_cost_usd\":0.225213,",
    "\"usage\":{\"input_tokens\":10,\"output_tokens\":81,\"output_tokens_details\":{\"thinking_tokens\":75}},",
    "\"is_error\":false,\"num_turns\":1,\"subtype\":\"success\",\"result\":\"ok\",\"type\":\"result\"}\n"
);

fn request(effort: Option<&str>) -> ProviderRequest {
    let input = RunInput {
        kind: WorkflowKind::Feasibility,
        intake: Intake {
            title: "CLI fixture".into(),
            description: [
                "SYNTHETIC TEST MATERIAL ONLY. A fictional container reports an unavailable ",
                "sensor rather than inventing a reading. This exists to exercise the CLI path.",
            ]
            .concat(),
            technical_field: "temperature sensing".into(),
            ..Default::default()
        },
        provider: ProviderSnapshot {
            kind: ProviderKind::ClaudeCode,
            model: "claude-test".into(),
            endpoint: None,
        },
        prompts: PromptSnapshot {
            version: "fixture".into(),
            sha256: "0".repeat(64),
        },
        authorization: Authorization {
            synthetic_data: true,
            ..Default::default()
        },
        upstream: Upstream::default(),
        regeneration: None,
        provider_plan: Vec::new(),
    };
    ProviderRequest {
        run_id: RunId("run".into()),
        attempt_id: AttemptId("attempt".into()),
        stage: Stage::TechnicalRestatement,
        input,
        system_prompt: "system".into(),
        user_prompt: "user".into(),
        prior_outputs: Vec::new(),
        effort: effort.map(str::to_string),
    }
}

/// Writes a program that does what the test needs, portably.
fn fake_program(body: &str, extension: &str) -> (tempfile::TempDir, String) {
    let dir = tempdir().unwrap();
    #[cfg(windows)]
    {
        let path = dir.path().join(format!("fake{extension}.cmd"));
        std::fs::write(&path, body).unwrap();
        (dir, path.to_string_lossy().to_string())
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.path().join(format!("fake{extension}.sh"));
        std::fs::write(&path, body).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        (dir, path.to_string_lossy().to_string())
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (body, extension);
        panic!("no fake-program support on this platform")
    }
}

#[test]
fn parses_the_real_envelope_past_the_warning_line() {
    let (text, usage) = parse_claude_result(REAL_ENVELOPE).unwrap();
    assert_eq!(text, "ok");
    assert_eq!(usage.input_tokens, Some(10));
    assert_eq!(usage.output_tokens, Some(81));
}

#[test]
fn a_reported_error_is_a_refusal_not_an_answer() {
    let stdout = r#"{"type":"result","is_error":true,"result":"Credit balance is too low"}"#;
    let error = parse_claude_result(stdout).unwrap_err();
    assert!(format!("{error}").contains("Credit balance is too low"));
}

#[test]
fn no_envelope_says_what_it_saw_instead() {
    let error = parse_claude_result("command not found\n").unwrap_err();
    assert!(format!("{error}").contains("no result envelope"));
    assert!(format!("{error}").contains("command not found"));
}

#[test]
fn an_empty_answer_is_incomplete() {
    let error = parse_claude_result(r#"{"type":"result","result":"   "}"#).unwrap_err();
    assert!(format!("{error}").contains("no text"));
}

#[test]
fn secrets_are_removed_from_anything_printed() {
    let redacted = redact("auth failed for sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789");
    assert!(!redacted.contains("sk-ant-api03"));
    assert!(redacted.contains("[redacted]"));
}

#[tokio::test]
async fn a_fake_cli_answers_and_reports_usage() {
    #[cfg(windows)]
    let body = concat!(
        "@echo off\n",
        "echo {\"type\":\"result\",\"result\":\"hello\",\"usage\":{\"input_tokens\":3,\"output_tokens\":4}}\n"
    );
    #[cfg(unix)]
    let body = concat!(
        "#!/bin/sh\n",
        "printf '%s\\n' '{\"type\":\"result\",\"result\":\"hello\",\"usage\":{\"input_tokens\":3,\"output_tokens\":4}}'\n"
    );
    let (_dir, program) = fake_program(body, "ok");
    let provider = ClaudeCliProvider::with_program(program, "claude-test").unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let response = provider
        .generate(
            request(None),
            tokio_util::sync::CancellationToken::new(),
            tx,
        )
        .await
        .unwrap();
    assert_eq!(response.text, "hello");
    assert_eq!(response.usage.input_tokens, Some(3));
    assert_eq!(response.usage.output_tokens, Some(4));
}

#[tokio::test]
async fn a_non_zero_exit_carries_the_code_and_the_reason() {
    #[cfg(windows)]
    let body = "@echo off\necho the login has expired 1>&2\nexit /b 3\n";
    #[cfg(unix)]
    let body = "#!/bin/sh\necho the login has expired >&2\nexit 3\n";
    let (_dir, program) = fake_program(body, "fail");
    let provider = ClaudeCliProvider::with_program(program, "claude-test").unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let error = provider
        .generate(
            request(None),
            tokio_util::sync::CancellationToken::new(),
            tx,
        )
        .await
        .unwrap_err();
    let text = format!("{error}");
    assert!(text.contains("code 3"), "got {text}");
    assert!(text.contains("login has expired"), "got {text}");
}

#[tokio::test]
async fn a_program_that_never_answers_times_out() {
    #[cfg(windows)]
    let body = "@echo off\nping -n 30 127.0.0.1 >nul\n";
    #[cfg(unix)]
    let body = "#!/bin/sh\nsleep 30\n";
    let (_dir, program) = fake_program(body, "slow");
    let provider = ClaudeCliProvider::with_program(program, "claude-test")
        .unwrap()
        .with_timeout(Duration::from_millis(300));
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let error = provider
        .generate(
            request(None),
            tokio_util::sync::CancellationToken::new(),
            tx,
        )
        .await
        .unwrap_err();
    assert!(format!("{error}").contains("did not answer"), "got {error}");
}

#[tokio::test]
async fn a_missing_program_says_how_to_fix_it() {
    let provider =
        ClaudeCliProvider::with_program("definitely-not-a-real-program-xyz", "claude-test")
            .unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let error = provider
        .generate(
            request(None),
            tokio_util::sync::CancellationToken::new(),
            tx,
        )
        .await
        .unwrap_err();
    let text = format!("{error}");
    assert!(text.contains("PFL_CLAUDE_CLI"), "got {text}");
}

#[tokio::test]
async fn an_effort_word_is_passed_through_to_the_program() {
    // The fake program records its arguments, so the test can read them back.
    let dir = tempdir().unwrap();
    let log = dir.path().join("args.txt");
    let log_path = log.to_string_lossy().to_string();
    #[cfg(windows)]
    let body = format!(
        "@echo off\necho %* > \"{log_path}\"\necho {{\"type\":\"result\",\"result\":\"ok\"}}\n"
    );
    #[cfg(unix)]
    let body = format!(
        "#!/bin/sh\necho \"$@\" > '{log_path}'\nprintf '%s\\n' '{{\"type\":\"result\",\"result\":\"ok\"}}'\n"
    );
    let (_program_dir, program) = fake_program(&body, "args");
    let provider = ClaudeCliProvider::with_program(program, "claude-test").unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    provider
        .generate(
            request(Some("high")),
            tokio_util::sync::CancellationToken::new(),
            tx,
        )
        .await
        .unwrap();
    let recorded = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(recorded.contains("--effort"), "got {recorded}");
    assert!(recorded.contains("high"), "got {recorded}");
    assert!(recorded.contains("--output-format"), "got {recorded}");
    assert!(recorded.contains("json"), "got {recorded}");
}
