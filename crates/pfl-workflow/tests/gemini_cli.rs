//! The Gemini CLI transport, against its documented JSON and a fake program.
//!
//! This CLI answers with one JSON object: `response` on success, `error.message` when it refuses.
//! Authentication is Google's, so the live check is opt-in (`PFL_GEMINI_LIVE=1`) and recorded
//! rather than assumed.
use pfl_domain::*;
use pfl_workflow::cli::{GeminiCliProvider, parse_gemini_json};
use tempfile::tempdir;

fn request() -> ProviderRequest {
    let input = RunInput {
        kind: WorkflowKind::Feasibility,
        intake: Intake {
            title: "Gemini fixture".into(),
            description:
                "SYNTHETIC TEST MATERIAL ONLY. A fictional container reports an unavailable sensor."
                    .into(),
            technical_field: "temperature sensing".into(),
            ..Default::default()
        },
        provider: ProviderSnapshot {
            kind: ProviderKind::GeminiCli,
            model: "gemini-test".into(),
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
fn parses_a_successful_answer_with_usage() {
    let (text, usage) =
        parse_gemini_json(r#"{"response":"ok","stats":{"tokens":{"input":3,"output":4}}}"#)
            .unwrap();
    assert_eq!(text, "ok");
    assert_eq!(usage.input_tokens, Some(3));
    assert_eq!(usage.output_tokens, Some(4));
}

#[test]
fn the_auth_refusal_is_reported_as_an_error_naming_auth() {
    let stdout = r#"{
  "session_id": "x",
  "error": {
    "type": "Error",
    "message": "Please set an Auth method in your settings.json or specify one of the following environment variables before running: GEMINI_API_KEY",
    "code": 41
  }
}"#;
    let error = parse_gemini_json(stdout).unwrap_err();
    let text = format!("{error}");
    assert!(text.contains("GEMINI_API_KEY"), "got {text}");
}

#[test]
fn an_empty_response_is_incomplete() {
    let error = parse_gemini_json(r#"{"response":"   "}"#).unwrap_err();
    assert!(format!("{error}").contains("no text"));
}

#[test]
fn something_that_is_not_json_says_what_it_saw() {
    let error = parse_gemini_json("command not found\n").unwrap_err();
    assert!(format!("{error}").contains("no JSON"));
}

#[tokio::test]
async fn a_fake_program_answers() {
    #[cfg(windows)]
    let body = "@echo off\r\necho {\"response\":\"hello\",\"stats\":{\"tokens\":{\"input\":5,\"output\":2}}}\r\n";
    #[cfg(unix)]
    let body = "#!/bin/sh\nprintf '%s\\n' '{\"response\":\"hello\",\"stats\":{\"tokens\":{\"input\":5,\"output\":2}}}'\n";
    let (_dir, program) = fake_program(body, "ok");
    let provider = GeminiCliProvider::with_program(program, "gemini-test").unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let response = provider
        .generate(request(), tokio_util::sync::CancellationToken::new(), tx)
        .await
        .unwrap();
    assert_eq!(response.text, "hello");
    assert_eq!(response.usage.input_tokens, Some(5));
}

#[tokio::test]
async fn a_missing_program_says_how_to_sign_in() {
    let provider =
        GeminiCliProvider::with_program("definitely-not-a-real-program-xyz", "m").unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let error = provider
        .generate(request(), tokio_util::sync::CancellationToken::new(), tx)
        .await
        .unwrap_err();
    let text = format!("{error}");
    assert!(text.contains("gemini"), "got {text}");
}
