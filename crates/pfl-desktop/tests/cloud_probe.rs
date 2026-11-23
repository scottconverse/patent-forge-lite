//! The Cloud configuration path through the session actor.
//!
//! Configuring Cloud installs the key in memory and answers without contacting any
//! provider: no request is made merely by configuring. This closes a coverage gap —
//! nothing at the session layer exercised `ConfigureCloud` before.
use pfl_desktop::session::{ApplicationSession, Mode, Reply, Request};
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::ReplayProvider;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn drain_until(session: &ApplicationSession, label: &str) -> Reply {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        for reply in session.drain() {
            if matches!(reply, Reply::Configured(_) | Reply::Error(_)) {
                return reply;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("no reply to {label} within 10s");
}

#[test]
fn configuring_cloud_replies_without_contacting_a_provider() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let session =
        ApplicationSession::with_replay_provider(store, Arc::new(ReplayProvider::all_workflows()))
            .unwrap();

    session
        .request(Request::ConfigureCloud {
            model: "claude-mock".into(),
            key: "sk-ant-session-only".into(),
        })
        .unwrap();
    match drain_until(&session, "ConfigureCloud") {
        Reply::Configured(mode) => {
            assert_eq!(
                mode,
                Mode::Cloud {
                    model: "claude-mock".into()
                }
            );
        }
        Reply::Error(error) => panic!("configure Cloud failed: {error}"),
        other => panic!("unexpected reply {other:?}"),
    }

    // An empty key is refused rather than configuring a provider that cannot work.
    session
        .request(Request::ConfigureCloud {
            model: "claude-mock".into(),
            key: "   ".into(),
        })
        .unwrap();
    match drain_until(&session, "empty key") {
        Reply::Error(error) => assert!(
            error.contains("API key"),
            "the refusal should name the missing key, got: {error}"
        ),
        Reply::Configured(_) => panic!("an empty key must not configure Cloud"),
        other => panic!("unexpected reply {other:?}"),
    }
}
