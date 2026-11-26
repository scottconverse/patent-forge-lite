//! T3: assignments, provider settings and operator connections.
//!
//! These are the rows the Models page will write. The tests that matter are the ones about what
//! happens to a database written by a build that never had these tables, and about a key that must
//! never be stored.
use pfl_domain::*;
use pfl_storage::SqliteStore;
use rusqlite::Connection;
use tempfile::tempdir;

fn project() -> Project {
    Project {
        id: ProjectId("synthetic-project".into()),
        intake: Intake {
            title: "Synthetic café 🧪".into(),
            description: "Synthetic description".into(),
            technical_field: "Synthetic testing".into(),
            context: InventionContext {
                problem_solved: "Synthetic problem".into(),
                how_it_works: "Synthetic mechanism".into(),
                ai_components: "Synthetic AI".into(),
                three_d_print_components: "Synthetic physical".into(),
                what_is_novel: "Synthetic novelty".into(),
                current_alternatives: "Synthetic alternatives".into(),
                what_is_built: "Synthetic built".into(),
                what_to_protect: "Synthetic scope".into(),
                additional_notes: "Synthetic notes".into(),
            },
        },
        created_at_ms: 1,
        updated_at_ms: 1,
    }
}

fn assignment(surface: &str, rank: u8, provider: &str, model: Option<&str>) -> ModelAssignment {
    ModelAssignment {
        surface: surface.into(),
        rank,
        provider: provider.into(),
        model: model.map(str::to_string),
        effort: None,
    }
}

#[test]
fn an_assignment_round_trips_with_its_ranks_in_order() {
    let dir = tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("lite.sqlite3")).unwrap();
    let p = store.create_project(&project()).unwrap();

    store
        .set_assignment(
            &p.id,
            &assignment("stage-prior-art", 0, "local-ollama", None),
        )
        .unwrap();
    store
        .set_assignment(&p.id, &assignment("stage-prior-art", 1, "claude", None))
        .unwrap();
    store
        .set_assignment(&p.id, &assignment("stage-patentability", 0, "codex", None))
        .unwrap();

    let all = store.assignments(&p.id).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].surface, "stage-patentability");
    assert_eq!(all[1].surface, "stage-prior-art");
    assert_eq!(all[1].rank, 0);
    assert_eq!(all[2].rank, 1);
    assert_eq!(all[2].provider, "claude");
}

#[test]
fn setting_the_same_rank_again_replaces_it_rather_than_duplicating() {
    let dir = tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("lite.sqlite3")).unwrap();
    let p = store.create_project(&project()).unwrap();

    store
        .set_assignment(&p.id, &assignment("stage-landscape", 0, "claude", None))
        .unwrap();
    store
        .set_assignment(
            &p.id,
            &assignment("stage-landscape", 0, "openai", Some("gpt-x")),
        )
        .unwrap();

    let all = store.assignments(&p.id).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].provider, "openai");
    assert_eq!(all[0].model.as_deref(), Some("gpt-x"));
}

#[test]
fn a_fourth_rank_is_refused() {
    let dir = tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("lite.sqlite3")).unwrap();
    let p = store.create_project(&project()).unwrap();
    let err = store
        .set_assignment(&p.id, &assignment("stage-landscape", 3, "claude", None))
        .unwrap_err();
    assert!(format!("{err}").contains("at most three ranks"));
}

#[test]
fn clearing_a_surface_removes_every_rank_and_nothing_else() {
    let dir = tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("lite.sqlite3")).unwrap();
    let p = store.create_project(&project()).unwrap();
    for rank in 0..=1 {
        store
            .set_assignment(&p.id, &assignment("stage-landscape", rank, "claude", None))
            .unwrap();
    }
    store
        .set_assignment(&p.id, &assignment("stage-prior-art", 0, "codex", None))
        .unwrap();

    assert_eq!(
        store.clear_assignments(&p.id, "stage-landscape").unwrap(),
        2
    );
    let left = store.assignments(&p.id).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].surface, "stage-prior-art");
}

#[test]
fn a_database_written_before_these_tables_existed_opens_unassigned() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lite.sqlite3");
    {
        let store = SqliteStore::open(&path).unwrap();
        let p = store.create_project(&project()).unwrap();
        store
            .set_assignment(&p.id, &assignment("stage-landscape", 0, "claude", None))
            .unwrap();
    }
    // Simulate the older build: the tables simply are not there.
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "DROP TABLE model_assignments; DROP TABLE provider_settings; DROP TABLE api_connections;",
            )
            .unwrap();
    }
    let store = SqliteStore::open(&path).unwrap();
    let projects = store.list_projects().unwrap();
    assert_eq!(projects.len(), 1);
    assert!(store.assignments(&projects[0].id).unwrap().is_empty());
    assert_eq!(
        store.provider_settings().unwrap(),
        ProviderSettings::default()
    );
    assert!(store.connections().unwrap().is_empty());
}

/// The product has no "delete project" action yet, so this removes the project row the way a
/// future one would: dependents first. The claim under test is narrower and still worth making —
/// the new table carries its own cascade, so no assignment outlives the project it belongs to.
#[test]
fn assignments_cascade_when_their_project_row_goes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lite.sqlite3");
    let store = SqliteStore::open(&path).unwrap();
    let p = store.create_project(&project()).unwrap();
    store
        .set_assignment(&p.id, &assignment("stage-landscape", 0, "claude", None))
        .unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "DELETE FROM intake_revisions WHERE project_id=?1",
            [&p.id.0],
        )
        .unwrap();
    connection
        .execute("DELETE FROM projects WHERE id=?1", [&p.id.0])
        .unwrap();
    let remaining: i64 = connection
        .query_row("SELECT count(*) FROM model_assignments", [], |r| r.get(0))
        .unwrap();
    assert_eq!(remaining, 0, "the new table must cascade with its project");
}

#[test]
fn provider_settings_default_and_round_trip() {
    let dir = tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("lite.sqlite3")).unwrap();
    assert_eq!(
        store.provider_settings().unwrap(),
        ProviderSettings::default()
    );

    let settings = ProviderSettings {
        openai_base_url: Some("http://127.0.0.1:1234/v1".into()),
        anthropic_endpoint: None,
        local_endpoint_override: Some("http://127.0.0.1:9999".into()),
        wall_budget_ms: Some(90_000),
    };
    store.save_provider_settings(&settings).unwrap();
    assert_eq!(store.provider_settings().unwrap(), settings);
}

#[test]
fn connections_can_be_created_listed_and_deleted() {
    let dir = tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("lite.sqlite3")).unwrap();
    let connection = ApiConnection {
        id: "conn-1".into(),
        name: "My gateway".into(),
        base_url: "https://gateway.example/v1".into(),
        model: Some("some-model".into()),
        key_ref: Some("session-key-1".into()),
    };
    store.save_connection(&connection).unwrap();
    assert_eq!(store.connections().unwrap(), vec![connection.clone()]);

    store.delete_connection("conn-1").unwrap();
    assert!(store.connections().unwrap().is_empty());
    assert!(
        store.delete_connection("conn-1").is_err(),
        "deleting twice is not silent"
    );
}

#[test]
fn a_connection_row_carries_no_key() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lite.sqlite3");
    let store = SqliteStore::open(&path).unwrap();
    store
        .save_connection(&ApiConnection {
            id: "conn-secret".into(),
            name: "Gateway".into(),
            base_url: "https://gateway.example/v1".into(),
            model: None,
            // Only a name for the in-session credential, never the credential.
            key_ref: Some("session-key-1".into()),
        })
        .unwrap();
    let connection = Connection::open(&path).unwrap();
    let payload: String = connection
        .query_row(
            "SELECT payload FROM api_connections WHERE id='conn-secret'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(payload.contains("session-key-1"));
    for forbidden in ["sk-ant-", "Bearer ", "api_key=", "apikey", "secret="] {
        assert!(
            !payload.contains(forbidden),
            "a connection row must not carry a credential, found {forbidden}"
        );
    }
}

#[test]
fn a_connection_survives_a_reopen() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lite.sqlite3");
    {
        let store = SqliteStore::open(&path).unwrap();
        store
            .save_connection(&ApiConnection {
                id: "conn-2".into(),
                name: "LM Studio".into(),
                base_url: "http://127.0.0.1:1234/v1".into(),
                model: Some("qwen".into()),
                key_ref: None,
            })
            .unwrap();
    }
    let store = SqliteStore::open(&path).unwrap();
    let all = store.connections().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].base_url, "http://127.0.0.1:1234/v1");
}
#[test]
fn a_login_row_starts_open_and_closes_when_it_is_saved() {
    let dir = tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("lite.sqlite3")).unwrap();
    let login = store.create_login("claude", now_ms() + 600_000).unwrap();
    assert_eq!(login.status, "starting");
    assert!(store.open_login("claude").unwrap().is_some());
    assert!(store.open_login("codex").unwrap().is_none());

    let mut saved = login.clone();
    saved.status = "awaiting_user".into();
    saved.url = Some("https://claude.com/cai/oauth/authorize?x=1".into());
    store.save_login(&saved).unwrap();
    let read = store.open_login("claude").unwrap().unwrap();
    assert_eq!(read.status, "awaiting_user");
    assert_eq!(
        read.url.as_deref(),
        Some("https://claude.com/cai/oauth/authorize?x=1")
    );

    saved.status = "done".into();
    store.save_login(&saved).unwrap();
    assert!(
        store.open_login("claude").unwrap().is_none(),
        "a finished attempt is not open"
    );
    assert_eq!(
        store.latest_login("claude").unwrap().unwrap().status,
        "done"
    );
}

#[test]
fn an_expired_attempt_is_not_open() {
    let dir = tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("lite.sqlite3")).unwrap();
    store.create_login("codex", now_ms() - 1).unwrap();
    assert!(store.open_login("codex").unwrap().is_none());
    assert!(store.latest_login("codex").unwrap().is_some());
}

#[test]
fn saving_a_login_that_does_not_exist_is_an_error() {
    let dir = tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("lite.sqlite3")).unwrap();
    let ghost = ProviderLogin {
        id: 999,
        provider: "claude".into(),
        status: "done".into(),
        url: None,
        code: None,
        detail: String::new(),
        expires_at_ms: now_ms(),
    };
    assert!(store.save_login(&ghost).is_err());
}
