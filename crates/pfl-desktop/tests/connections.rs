//! Adding a custom connection through the session actor.
//!
//! The form on the Models page sends `AddConnection`. This exercises the same path without a window:
//! the record is saved, the reply carries the list, a nameless connection is refused, and nothing is
//! contacted by saving one.
use pfl_desktop::session::{ApplicationSession, Reply, Request};
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::ReplayProvider;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn drain_until(session: &ApplicationSession, label: &str) -> Reply {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        for reply in session.drain() {
            if matches!(reply, Reply::Connections(_) | Reply::Error(_)) {
                return reply;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("no reply to {label} within 10s");
}

fn session() -> ApplicationSession {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    ApplicationSession::with_replay_provider(store, Arc::new(ReplayProvider::all_workflows()))
        .unwrap()
}

#[test]
fn a_connection_is_saved_listed_and_never_contacted() {
    let session = session();
    session
        .request(Request::AddConnection {
            name: "Gateway".into(),
            base_url: "http://127.0.0.1:1234/v1".into(),
            model: Some("local-model".into()),
        })
        .unwrap();
    match drain_until(&session, "AddConnection") {
        Reply::Connections(rows) => {
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].name, "Gateway");
            assert_eq!(rows[0].base_url, "http://127.0.0.1:1234/v1");
            assert_eq!(rows[0].model.as_deref(), Some("local-model"));
            assert!(rows[0].id.starts_with("conn-"), "the id is addressable");
            assert_eq!(rows[0].key_ref, None, "no credential is stored with it");
        }
        Reply::Error(error) => panic!("adding a connection failed: {error}"),
        other => panic!("unexpected reply {other:?}"),
    }

    session.request(Request::ListConnections).unwrap();
    match drain_until(&session, "ListConnections") {
        Reply::Connections(rows) => assert_eq!(rows.len(), 1, "listed once"),
        other => panic!("unexpected reply {other:?}"),
    }
}

#[test]
fn a_connection_without_a_name_or_url_is_refused_and_not_saved() {
    let session = session();
    for (name, url) in [("", "http://127.0.0.1:1/v1"), ("Gateway", "  ")] {
        session
            .request(Request::AddConnection {
                name: name.into(),
                base_url: url.into(),
                model: None,
            })
            .unwrap();
        match drain_until(&session, "refused") {
            Reply::Error(error) => {
                let text = error.to_string();
                assert!(text.contains("name"), "got {text}");
            }
            other => panic!("unexpected reply {other:?}"),
        }
    }
    session.request(Request::ListConnections).unwrap();
    match drain_until(&session, "ListConnections") {
        Reply::Connections(rows) => assert!(rows.is_empty(), "nothing was saved"),
        other => panic!("unexpected reply {other:?}"),
    }
}
