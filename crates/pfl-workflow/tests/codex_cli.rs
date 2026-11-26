//! The Codex CLI transport, against the real event stream and fake programs.
//!
//! Codex answers with JSONL events, not one envelope: the answer is the last `agent_message`
//! inside an `item.completed`, the usage is on `turn.completed`, and a failed turn arrives as an
//! event whose reason is nested inside a JSON *string*. The fixture below is a captured run from
//! this machine (`gpt-6.1-sol`), trimmed to the events that matter.
use pfl_domain::*;
use pfl_workflow::cli::{CodexCliProvider, parse_codex_jsonl};
use std::time::Duration;
use tempfile::tempdir;

const CAPTURED: &str = concat!(
    r#"{"type":"thread.started","thread_id":"01a2171f-672f-7b00-89e9-92ec7f487e13"}"#,
    "\n",
    r#"{"type":"turn.started"}"#,
    "\n",
    r#"{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"ok"}}"#,
    "\n",
    r#"{"type":"turn.completed","usage":{"input_tokens":24845,"cached_input_tokens":0,"output_tokens":5,"reasoning_output_tokens":0}}"#,
    "\n"
);

fn request() -> ProviderRequest {
    let input = RunInput {
        kind: WorkflowKind::Feasibility,
        intake: Intake {
            title: "Codex fixture".into(),
            description: [
                "SYNTHETIC TEST MATERIAL ONLY. A fictional container reports an unavailable ",
                "sensor rather than inventing a reading. This exercises the Codex CLI path.",
            ]
            .concat(),
            technical_field: "temperature sensing".into(),
            ..Default::default()
        },
        provider: ProviderSnapshot {
            kind: ProviderKind::Codex,
            model: "gpt-6.1-sol".into(),
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
    };
    ProviderRequest {
        run_id: RunId("run".into()),
        attempt_id: AttemptId("attempt".into()),
        stage: Stage::TechnicalRestatement,
        input,
        system_prompt: "system".into(),
        user_prompt: "user".into(),
        prior_outputs: Vec::new(),
        effort: None,
    }
}

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
fn parses_the_captured_event_stream() {
    let (text, usage) = parse_codex_jsonl(CAPTURED).unwrap();
    assert_eq!(text, "ok");
    assert_eq!(usage.input_tokens, Some(24845));
    assert_eq!(usage.output_tokens, Some(5));
}

#[test]
fn reasoning_and_tool_events_are_not_the_answer() {
    let stdout = concat!(
        r#"{"type":"thread.started","thread_id":"x"}"#,
        "\n",
        r#"{"type":"item.completed","item":{"type":"reasoning","text":"thinking about it"}}"#,
        "\n",
        r#"{"type":"item.completed","item":{"type":"tool_call","name":"shell"}}"#,
        "\n"
    );
    let error = parse_codex_jsonl(stdout).unwrap_err();
    assert!(
        format!("{error}").contains("without an agent message"),
        "got {error}"
    );
}

#[test]
fn a_failed_turn_reports_the_nested_reason() {
    let stdout = concat!(
        r#"{"type":"turn.started"}"#,
        "\n",
        r#"{"type":"turn.failed","error":{"message":"{\"type\":\"error\",\"status\":400,\"error\":{\"type\":\"invalid_request_error\",\"message\":\"The 'gpt-5-codex' model is not supported when using Codex with a ChatGPT account.\"}}"}}"#,
        "\n"
    );
    let error = parse_codex_jsonl(stdout).unwrap_err();
    let text = format!("{error}");
    assert!(text.contains("not supported"), "got {text}");
    assert!(
        !text.contains("\\\""),
        "the nested JSON should be unwrapped: {text}"
    );
}

#[test]
fn a_bare_error_event_is_a_refusal() {
    let error = parse_codex_jsonl(r#"{"type":"error","message":"unauthorized"}"#).unwrap_err();
    assert!(format!("{error}").contains("unauthorized"));
}

#[test]
fn something_that_is_not_an_event_stream_says_so() {
    let error = parse_codex_jsonl("codex: command not found\n").unwrap_err();
    let text = format!("{error}");
    assert!(text.contains("no event stream"), "got {text}");
    assert!(text.contains("command not found"), "got {text}");
}

#[tokio::test]
async fn a_fake_program_answers_and_receives_the_prompt_on_stdin() {
    let dir = tempdir().unwrap();
    let seen = dir.path().join("stdin.txt");
    let seen_path = seen.to_string_lossy().to_string();
    #[cfg(windows)]
    let body = format!(
        "@echo off\r\nmore > \"{seen_path}\"\r\necho {{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"hello\"}}}}\r\necho {{\"type\":\"turn.completed\",\"usage\":{{\"input_tokens\":7,\"output_tokens\":2}}}}\r\n"
    );
    #[cfg(unix)]
    let body = format!(
        "#!/bin/sh\ncat > '{seen_path}'\nprintf '%s\\n' '{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"hello\"}}}}'\nprintf '%s\\n' '{{\"type\":\"turn.completed\",\"usage\":{{\"input_tokens\":7,\"output_tokens\":2}}}}'\n"
    );
    let (_dir, program) = fake_program(&body, "ok");
    let provider = CodexCliProvider::with_program(program, "gpt-test").unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let response = provider
        .generate(request(), tokio_util::sync::CancellationToken::new(), tx)
        .await
        .unwrap();
    assert_eq!(response.text, "hello");
    assert_eq!(response.usage.input_tokens, Some(7));
    assert_eq!(response.usage.output_tokens, Some(2));
    let captured = std::fs::read_to_string(&seen).unwrap_or_default();
    assert!(
        captured.contains("SYSTEM INSTRUCTIONS"),
        "the prompt must reach the program on stdin; got {captured:?}"
    );
}

#[tokio::test]
async fn a_missing_program_says_how_to_fix_it() {
    let provider =
        CodexCliProvider::with_program("definitely-not-a-real-program-xyz", "gpt-test").unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let error = provider
        .generate(request(), tokio_util::sync::CancellationToken::new(), tx)
        .await
        .unwrap_err();
    let text = format!("{error}");
    assert!(text.contains("PFL_CODEX_CLI"), "got {text}");
    assert!(text.contains("codex login"), "got {text}");
}

#[tokio::test]
async fn a_program_that_never_answers_times_out() {
    #[cfg(windows)]
    let body = "@echo off\nping -n 30 127.0.0.1 >nul\n";
    #[cfg(unix)]
    let body = "#!/bin/sh\nsleep 30\n";
    let (_dir, program) = fake_program(body, "slow");
    let provider = CodexCliProvider::with_program(program, "gpt-test")
        .unwrap()
        .with_timeout(Duration::from_millis(300));
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let error = provider
        .generate(request(), tokio_util::sync::CancellationToken::new(), tx)
        .await
        .unwrap_err();
    assert!(format!("{error}").contains("did not answer"), "got {error}");
}
