//! Starting, polling and cancelling a sign-in, against fake programs.
use pfl_workflow::cli_login::{LoginManager, LoginState};
use std::time::Duration;
use tempfile::tempdir;

fn fake(body: &str, name: &str) -> (tempfile::TempDir, String) {
    let dir = tempdir().unwrap();
    #[cfg(windows)]
    {
        let path = dir.path().join(format!("{name}.cmd"));
        std::fs::write(&path, body).unwrap();
        (dir, path.to_string_lossy().to_string())
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.path().join(format!("{name}.sh"));
        std::fs::write(&path, body).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        (dir, path.to_string_lossy().to_string())
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (body, name);
        panic!("no fake-program support on this platform")
    }
}

#[tokio::test]
async fn an_attempt_reports_the_url_then_finishes() {
    #[cfg(windows)]
    let body = "@echo off\r\necho https://claude.com/cai/oauth/authorize?x=1\r\nping -n 3 127.0.0.1 >nul\r\nexit /b 0\r\n";
    #[cfg(unix)]
    let body = "#!/bin/sh\nprintf '%s\\n' 'If the browser could not open, visit: https://claude.com/cai/oauth/authorize?x=1'\nsleep 1\nexit 0\n";
    let (_dir, program) = fake(body, "signin");
    let manager = LoginManager::new();
    let started = manager.start_with("claude", &program, &[]).await.unwrap();
    // A very fast program can finish before the first poll; what must not vary is that the URL the
    // human needs is surfaced either way.
    assert!(
        matches!(started.state, LoginState::AwaitingUser | LoginState::Done),
        "got {:?}",
        started.state
    );
    assert_eq!(
        started.url.as_deref(),
        Some("https://claude.com/cai/oauth/authorize?x=1")
    );

    // Let the program finish, then ask again.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let finished = manager.poll(started.id).unwrap();
    assert_eq!(finished.state, LoginState::Done);
}
#[tokio::test]
async fn a_failing_program_reports_its_reason_redacted() {
    #[cfg(windows)]
    let body = "@echo off\r\necho the login was refused 1>&2\r\nexit /b 3\r\n";
    #[cfg(unix)]
    let body = "#!/bin/sh\necho 'the login was refused' >&2\nexit 3\n";
    let (_dir, program) = fake(body, "refuse");
    let manager = LoginManager::new();
    let started = manager.start_with("codex", &program, &[]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    let finished = manager.poll(started.id).unwrap();
    assert_eq!(finished.state, LoginState::Failed);
    assert!(
        finished.detail.contains("refused"),
        "got {:?}",
        finished.detail
    );
}
#[tokio::test]
async fn a_waiting_attempt_can_be_cancelled() {
    #[cfg(windows)]
    let body = "@echo off\r\necho https://claude.com/cai/oauth/authorize?x=1\r\nping -n 30 127.0.0.1 >nul\r\n";
    #[cfg(unix)]
    let body =
        "#!/bin/sh\nprintf '%s\\n' 'visit https://claude.com/cai/oauth/authorize?x=1'\nsleep 30\n";
    let (_dir, program) = fake(body, "slow");
    let manager = LoginManager::new();
    let started = manager.start_with("claude", &program, &[]).await.unwrap();
    assert_eq!(started.state, LoginState::AwaitingUser);
    let cancelled = manager.cancel(started.id).unwrap();
    assert_eq!(cancelled.state, LoginState::Cancelled);
}
#[tokio::test]
async fn one_attempt_per_provider_at_a_time() {
    #[cfg(windows)]
    let body = "@echo off\r\necho https://claude.com/cai/oauth/authorize?x=1\r\nping -n 30 127.0.0.1 >nul\r\n";
    #[cfg(unix)]
    let body =
        "#!/bin/sh\nprintf '%s\\n' 'visit https://claude.com/cai/oauth/authorize?x=1'\nsleep 30\n";
    let (_dir, program) = fake(body, "twice");
    let manager = LoginManager::new();
    let first = manager.start_with("claude", &program, &[]).await.unwrap();
    let second = manager.start_with("claude", &program, &[]).await;
    assert!(second.is_err(), "a second attempt must be refused");
    manager.cancel(first.id).unwrap();
    // Cancelled attempts may be retired, and then a new one is allowed.
    manager.retire(first.id).unwrap();
    let third = manager.start_with("claude", &program, &[]).await.unwrap();
    // Leave nothing running: a live child holds the reader's pipes open past the test.
    assert_eq!(
        manager.cancel(third.id).unwrap().state,
        LoginState::Cancelled
    );
}
#[tokio::test]
async fn the_url_is_found_even_when_the_program_is_slow_to_be_read() {
    // The buffer is drained by a background task; a URL arriving in a second read must still be
    // seen on a later poll.
    #[cfg(windows)]
    let body = "@echo off\r\nping -n 2 127.0.0.1 >nul\r\necho https://auth.openai.com/codex/device\r\necho Enter code: WXYZ-9876\r\nping -n 30 127.0.0.1 >nul\r\n";
    #[cfg(unix)]
    let body = "#!/bin/sh\nsleep 1\nprintf '%s\\n' 'https://auth.openai.com/codex/device'\nprintf '%s\\n' 'Enter code: WXYZ-9876'\nsleep 30\n";
    let (_dir, program) = fake(body, "late");
    let manager = LoginManager::new();
    let started = manager.start_with("codex", &program, &[]).await.unwrap();
    let later = manager.poll(started.id).unwrap();
    assert_eq!(
        later.url.as_deref(),
        Some("https://auth.openai.com/codex/device")
    );
    assert_eq!(later.code.as_deref(), Some("WXYZ-9876"));
    manager.cancel(started.id).unwrap();
}
