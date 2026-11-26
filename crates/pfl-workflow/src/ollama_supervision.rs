//! Starting an installed-but-stopped Ollama, and never touching one this application did not start.
//!
//! Two rules, both from the reference implementation:
//!
//! - **Never stop a server we did not start.** The application has no business ending something the
//!   operator or the system started.
//! - **Never load or unload a model.** Starting the server is allowed; choosing what it holds is not.
//!
//! The decision is a pure function so it can be tested without a process, and the effectful part
//! takes its probe and spawn as arguments so the same decision can be exercised with fakes.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Something already answers; leave it alone.
    AlreadyRunning,
    /// This application started it, and will not stop it later.
    Started,
    /// Nothing answers and the program is not installed.
    NotInstalled,
    /// Installed, but it did not come up.
    FailedToStart,
}

impl Decision {
    pub fn slug(self) -> &'static str {
        match self {
            Decision::AlreadyRunning => "already-running",
            Decision::Started => "started-by-this-app",
            Decision::NotInstalled => "not-installed",
            Decision::FailedToStart => "failed-to-start",
        }
    }
}

/// The decision, with no side effects.
pub fn decide(answering: bool, installed: bool, spawned_ok: bool) -> Decision {
    match (answering, installed, spawned_ok) {
        (true, _, _) => Decision::AlreadyRunning,
        (false, false, _) => Decision::NotInstalled,
        (false, true, true) => Decision::Started,
        (false, true, false) => Decision::FailedToStart,
    }
}

/// Runs a probe, tries to start the program when nothing answers, then probes again.
///
/// `probe` answers "is the endpoint serving?"; `spawn` starts the program and answers whether the
/// spawn itself succeeded. The second probe is what makes `Started` mean "it is up" rather than
/// "we called spawn".
pub async fn ensure<F, S>(mut probe: F, spawn: S, attempts: u32, wait: Duration) -> Decision
where
    F: FnMut() -> bool,
    S: FnOnce() -> bool,
{
    if probe() {
        return Decision::AlreadyRunning;
    }
    let installed = true;
    let spawned_ok = spawn();
    if !spawned_ok {
        return decide(false, installed, false);
    }
    for _ in 0..attempts {
        if probe() {
            return Decision::Started;
        }
        tokio::time::sleep(wait).await;
    }
    Decision::FailedToStart
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_decision_covers_every_case() {
        assert_eq!(decide(true, true, true), Decision::AlreadyRunning);
        assert_eq!(
            decide(true, false, false),
            Decision::AlreadyRunning,
            "running wins"
        );
        assert_eq!(decide(false, false, false), Decision::NotInstalled);
        assert_eq!(decide(false, true, true), Decision::Started);
        assert_eq!(decide(false, true, false), Decision::FailedToStart);
    }

    #[tokio::test]
    async fn a_running_server_is_never_spawned() {
        let mut spawned = false;
        let decision = ensure(
            || true,
            || {
                spawned = true;
                true
            },
            3,
            Duration::from_millis(1),
        )
        .await;
        assert_eq!(decision, Decision::AlreadyRunning);
        assert!(!spawned, "a server that answers must not be started again");
    }

    #[tokio::test]
    async fn a_spawn_that_never_answers_is_a_failure_not_a_success() {
        let decision = ensure(|| false, || true, 2, Duration::from_millis(1)).await;
        assert_eq!(
            decision,
            Decision::FailedToStart,
            "spawning is not the same as serving"
        );
    }

    #[tokio::test]
    async fn a_server_that_comes_up_after_the_spawn_is_reported_started() {
        let mut calls = 0;
        let decision = ensure(
            || {
                calls += 1;
                // First probe: down. The two after the spawn: the second answers.
                calls >= 3
            },
            || true,
            5,
            Duration::from_millis(1),
        )
        .await;
        assert_eq!(decision, Decision::Started);
    }

    #[tokio::test]
    async fn a_failed_spawn_is_reported_without_waiting() {
        let decision = ensure(|| false, || false, 5, Duration::from_millis(1)).await;
        assert_eq!(decision, Decision::FailedToStart);
    }
}
