use async_trait::async_trait;
use pfl_storage::SqliteStore;
use pfl_workflow::*;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    runtime::Handle,
    sync::{Notify, mpsc::UnboundedSender},
};

fn original_context() -> InventionContext {
    InventionContext {
        problem_solved: "  original-problem café  ".into(),
        how_it_works: "\noriginal-mechanism spring\n".into(),
        ai_components: " original-AI none ".into(),
        three_d_print_components: " original-physical stand ".into(),
        what_is_novel: " original-novel unassessed ".into(),
        current_alternatives: " original-alternative marker ".into(),
        what_is_built: " original-built demonstration ".into(),
        what_to_protect: " original-protection undecided ".into(),
        additional_notes: " original-notes 日本語 🧪 ".into(),
    }
}
// Independent reference labels/order from the legacy narrative builder.
const LEGACY_FIELDS: [(&str, &str); 9] = [
    ("Problem Solved", "original-problem café"),
    ("How It Works", "original-mechanism spring"),
    ("AI / ML Components", "original-AI none"),
    (
        "3D Printing / Physical Design Components",
        "original-physical stand",
    ),
    ("What I Believe Is Novel", "original-novel unassessed"),
    (
        "Current Alternatives / Prior Solutions",
        "original-alternative marker",
    ),
    ("What Has Been Built So Far", "original-built demonstration"),
    ("What I Want Protected", "original-protection undecided"),
    ("Additional Notes", "original-notes 日本語 🧪"),
];
fn assert_legacy_context(prompt: &str) {
    let mut end = 0;
    for (label, text) in LEGACY_FIELDS {
        let fragment = format!("**{label}:** {text}");
        let offset = prompt
            .find(&fragment)
            .unwrap_or_else(|| panic!("missing original narrative field {label}"));
        assert!(offset >= end, "legacy context order changed");
        end = offset + fragment.len();
    }
    assert!(!prompt.contains("edited-current-context"));
}

fn intake() -> Intake {
    Intake {
        title: "Synthetic café tray 日本語 🧪 <script>alert('x')</script>".into(),
        description: "A fictional mechanical tray has a spring marker and an imaginary docking stand. The marker changes position when the tray returns to the stand. This completely synthetic description exists only to exercise streaming, saved input, Unicode, cancellation, recovery, export and immutable run versioning. It makes no claim about novelty or any actual invention.".into(),
        technical_field: "Demonstration mechanics".into(),
        context: original_context(),
    }
}
fn authorization() -> Authorization {
    Authorization {
        synthetic_data: true,
        ..Default::default()
    }
}
fn request(project: &Project, provider: &dyn Provider, key: &str) -> NewRun {
    NewRun {
        id: RunId::new(),
        project_id: project.id.clone(),
        idempotency_key: key.into(),
        input: RunInput {
            kind: WorkflowKind::Feasibility,
            intake: project.intake.clone(),
            provider: provider.snapshot(),
            prompts: prompts::snapshot(WorkflowKind::Feasibility),
            authorization: authorization(),
            upstream: Upstream::default(),
            regeneration: None,
        },
        rerun_of: None,
        created_at_ms: now_ms(),
    }
}

/// A minimal run input for message-composition assertions.
fn run_input(intake: Intake) -> RunInput {
    RunInput {
        kind: WorkflowKind::Feasibility,
        intake,
        provider: ProviderSnapshot {
            kind: ProviderKind::Replay,
            model: "synthetic".into(),
            endpoint: None,
        },
        prompts: prompts::snapshot(WorkflowKind::Feasibility),
        authorization: authorization(),
        upstream: Upstream::default(),
        regeneration: None,
    }
}
async fn terminal(service: &WorkflowService, id: &RunId) -> RunSnapshot {
    // Generous but still bounded: six stages against a real SQLite file take under a second
    // here, but a loaded two-core CI runner with synchronous writes can be an order of
    // magnitude slower. A genuine hang still fails this assertion.
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let run = service.run(id.clone()).await.unwrap();
            if run.outcome.is_terminal() {
                return run;
            }
            tokio::time::sleep(Duration::from_millis(3)).await;
        }
    })
    .await
    .expect("workflow must reach a bounded terminal state")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replay_real_store_reconnect_report_and_idempotency_do_not_generate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lite.db");
    let provider = Arc::new(ReplayProvider::synthetic());
    let store = Arc::new(SqliteStore::open(&path).unwrap());
    let service = WorkflowService::new(store, provider.clone(), Handle::current());
    service.recover_interrupted().await.unwrap();
    let project = service.create_project(intake()).await.unwrap();
    let req = request(&project, provider.as_ref(), "once");
    let (a, b) = tokio::join!(service.start(req.clone()), service.start(req.clone()));
    let id = a.unwrap().id;
    assert_eq!(id, b.unwrap().id);
    let mut cursor = Subscribe {
        run_id: id.clone(),
        after_sequence: 0,
    };
    for _ in 0..5 {
        let events = service.events(cursor.clone()).await.unwrap();
        if let Some(last) = events.last() {
            cursor.after_sequence = last.sequence;
        }
        service.project(project.id.clone()).await.unwrap();
        service.run(id.clone()).await.unwrap();
    }
    let saved = terminal(&service, &id).await;
    assert_eq!(saved.outcome, RunOutcome::Completed);
    assert_eq!(saved.checkpoints.len(), 6);
    assert_eq!(provider.requests().len(), 6);
    for actual in provider.requests() {
        assert_eq!(actual.input.intake, project.intake);
        assert_legacy_context(&actual.user_prompt);
    }
    service
        .save_intake(
            project.id.clone(),
            Intake {
                context: InventionContext {
                    additional_notes: "edited-current-context".into(),
                    ..Default::default()
                },
                ..intake()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        service.run(id.clone()).await.unwrap().input.intake,
        project.intake
    );
    assert!(
        saved
            .checkpoints
            .iter()
            .all(|cp| cp.usage.input_tokens.is_none() && cp.usage.output_tokens.is_none())
    );
    assert_eq!(service.start(req).await.unwrap().id, id);
    let events = service
        .events(Subscribe {
            run_id: id.clone(),
            after_sequence: 0,
        })
        .await
        .unwrap();
    assert_eq!(events.len() as u64, saved.last_sequence);
    assert!(
        events
            .windows(2)
            .all(|p| p[0].sequence + 1 == p[1].sequence)
    );
    for stage in Stage::ALL {
        assert_eq!(
            events
                .iter()
                .filter(
                    |e| matches!(e.event, EventKind::CheckpointSaved { stage: s, .. } if s == stage)
                )
                .count(),
            1
        );
    }
    let md = export_markdown(&saved).unwrap();
    let html = export_html(&saved).unwrap();
    assert!(md.contains("café") && md.contains("日本語") && md.contains("🧪"));
    assert!(md.contains(&id.to_string()));
    assert!(md.contains("original-problem café") && md.contains("original-notes 日本語 🧪"));
    assert!(!md.contains("edited-current-context"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script>"));
    let output = dir.path().join("report.html");
    std::fs::write(&output, &html).unwrap();
    assert_eq!(std::fs::read_to_string(output).unwrap(), html);
    service.shutdown().await.unwrap();
    drop(service);
    let reopened = WorkflowService::new(
        Arc::new(SqliteStore::open(&path).unwrap()),
        provider.clone(),
        Handle::current(),
    );
    assert_eq!(
        reopened.report(id.clone()).await.unwrap(),
        saved.report.unwrap()
    );
    assert_eq!(reopened.history(project.id).await.unwrap().len(), 1);
    assert_eq!(provider.requests().len(), 6);
    assert!(
        reopened
            .events(Subscribe {
                run_id: id,
                after_sequence: saved.last_sequence
            })
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recovery_before_stage_one_and_after_prefix_uses_original_snapshot() {
    for prefix in [0, 2, 5, 6] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recover.db");
        let provider = Arc::new(ReplayProvider::synthetic().with_delay(Duration::ZERO));
        let store = SqliteStore::open(&path).unwrap();
        let original = Project {
            id: ProjectId::new(),
            intake: intake(),
            created_at_ms: 1,
            updated_at_ms: 1,
        };
        store.create_project(&original).unwrap();
        let new = request(&original, provider.as_ref(), "interrupted");
        let id = new.id.clone();
        store.create_run(&new).unwrap();
        let attempt = store.begin_attempt(&id, 2).unwrap();
        for stage in Stage::ALL.into_iter().take(prefix) {
            store
                .commit_checkpoint(
                    &id,
                    &attempt.id,
                    &Checkpoint {
                        stage,
                        attempt_id: attempt.id.clone(),
                        text: format!("saved stage {} café 🧪", stage.number()),
                        usage: Usage::default(),
                        completed_at_ms: 3,
                    },
                )
                .unwrap();
        }
        drop(store); // Simulate startup with durable partial state, not a live job.
        let store = Arc::new(SqliteStore::open(&path).unwrap());
        let service = WorkflowService::new(store.clone(), provider.clone(), Handle::current());
        assert_eq!(
            service.recover_interrupted().await.unwrap(),
            vec![id.clone()]
        );
        service
            .save_intake(
                original.id,
                Intake {
                    description: "edited later".into(),
                    context: InventionContext {
                        additional_notes: "edited-current-context".into(),
                        ..Default::default()
                    },
                    ..intake()
                },
            )
            .await
            .unwrap();
        service
            .resume(
                id.clone(),
                Authorization {
                    synthetic_data: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let resumed = terminal(&service, &id).await;
        assert_eq!(resumed.outcome, RunOutcome::Completed);
        assert_eq!(resumed.id, id);
        assert_eq!(resumed.attempts.len(), 2);
        assert_eq!(resumed.checkpoints.len(), 6);
        assert_eq!(provider.requests().len(), 6 - prefix);
        assert!(
            provider
                .requests()
                .iter()
                .all(|r| r.input.intake == original.intake)
        );
        for actual in provider.requests() {
            assert_legacy_context(&actual.user_prompt);
        }
        assert_eq!(resumed.input.intake, original.intake);
        let exported = export_markdown(&resumed).unwrap();
        assert!(exported.contains("original-notes 日本語 🧪"));
        assert!(!exported.contains("edited-current-context"));
        for cp in resumed.checkpoints.iter().take(prefix) {
            assert!(cp.text.starts_with("saved stage"));
        }
        assert!(service.recover_interrupted().await.is_err());
        service.shutdown().await.unwrap();
    }
}

struct GatedProvider {
    requests: Mutex<Vec<(Stage, AttemptId)>>,
    blocked: Notify,
    release: Notify,
    block_once: AtomicBool,
    active: AtomicUsize,
    maximum: AtomicUsize,
}
struct Active<'a>(&'a AtomicUsize);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
impl GatedProvider {
    fn new() -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            blocked: Notify::new(),
            release: Notify::new(),
            block_once: AtomicBool::new(true),
            active: AtomicUsize::new(0),
            maximum: AtomicUsize::new(0),
        }
    }
}
#[async_trait]
impl Provider for GatedProvider {
    fn snapshot(&self) -> ProviderSnapshot {
        ProviderSnapshot {
            kind: ProviderKind::Replay,
            model: "gated-synthetic".into(),
            endpoint: None,
        }
    }
    async fn generate(
        &self,
        request: ProviderRequest,
        _: CancellationToken,
        tx: UnboundedSender<String>,
    ) -> Result<ProviderResponse> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(active, Ordering::SeqCst);
        let _guard = Active(&self.active);
        self.requests
            .lock()
            .unwrap()
            .push((request.stage, request.attempt_id));
        if request.stage == Stage::PriorArt && self.block_once.swap(false, Ordering::SeqCst) {
            self.blocked.notify_one();
            // Deliberately ignore the token: runner must drop this future and
            // prevent its stale response from reaching durable storage.
            self.release.notified().await;
        }
        let text = format!("Recorded stage {} café 🧪", request.stage.number());
        let _ = tx.send(text.clone());
        Ok(ProviderResponse {
            text,
            usage: Usage {
                input_tokens: Some(42),
                output_tokens: None,
            },
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_acknowledges_dropped_late_provider_before_replacement() {
    let provider = Arc::new(GatedProvider::new());
    let service = WorkflowService::new(
        Arc::new(SqliteStore::in_memory().unwrap()),
        provider.clone(),
        Handle::current(),
    );
    let p = service.create_project(intake()).await.unwrap();
    let started = service
        .start(request(&p, provider.as_ref(), "gated"))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), provider.blocked.notified())
        .await
        .unwrap();
    assert!(
        service
            .resume(
                started.id.clone(),
                Authorization {
                    synthetic_data: true,
                    ..Default::default()
                }
            )
            .await
            .is_err()
    );
    let cancelled = service.cancel(started.id.clone()).await.unwrap();
    assert_eq!(cancelled.outcome, RunOutcome::Cancelled);
    assert_eq!(cancelled.checkpoints.len(), 1);
    assert_eq!(provider.active.load(Ordering::SeqCst), 0);
    provider.release.notify_one();
    service
        .resume(
            started.id.clone(),
            Authorization {
                synthetic_data: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let complete = terminal(&service, &started.id).await;
    assert_eq!(complete.outcome, RunOutcome::Completed);
    assert_eq!(complete.attempts.len(), 2);
    let requests = provider.requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.0 == Stage::TechnicalRestatement)
            .count(),
        1
    );
    assert_eq!(requests.len(), 7);
    assert_eq!(provider.maximum.load(Ordering::SeqCst), 1);
}

/// The paid gate itself, with the service actually holding a Cloud provider.
///
/// The previous version of this test used a replay service and rewrote the request's provider
/// to Cloud, so the refusal came from the provider-snapshot mismatch and proved nothing about
/// consent. Here the service *is* a Cloud provider pointed at a loopback stand-in, so only the
/// paid gate can refuse, and the stand-in proves no request escaped.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn paid_paths_declined_or_approved_are_blocked_before_any_provider_request() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let counter = calls.clone();
    tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/v1/messages",
            axum::routing::post(move || {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    (
                        [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                        concat!(
                            "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":7}}}\n\n",
                            "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"Loopback stage output.\"}}\n\n",
                            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":11}}\n\n",
                            "data: {\"type\":\"message_stop\"}\n\n"
                        ),
                    )
                }
            }),
        );
        axum::serve(listener, app).await.unwrap();
    });
    let cloud = Arc::new(
        CloudProvider::with_endpoint("sk-ant-test", "claude-mock", format!("http://{address}"))
            .unwrap(),
    );
    let store = Arc::new(SqliteStore::in_memory().unwrap());
    let service = WorkflowService::new(store.clone(), cloud.clone(), Handle::current());
    let p = service.create_project(intake()).await.unwrap();

    // Declined: refused by the paid gate, and the stand-in is never contacted.
    let mut declined = request(&p, cloud.as_ref(), "paid-declined");
    declined.input.authorization.paid_provider = false;
    let refused = service
        .execute(Command::Start(declined))
        .await
        .expect_err("an unapproved paid run must be refused");
    assert!(
        refused.to_string().contains("paid-provider authorization"),
        "the refusal must come from the paid gate, got: {refused}"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "a declined paid run must not reach the provider"
    );

    // Approved: the same request runs and every stage reaches the stand-in.
    let mut approved = request(&p, cloud.as_ref(), "paid-approved");
    approved.input.authorization.paid_provider = true;
    let run = service
        .execute(Command::Start(approved))
        .await
        .expect("an approved paid run should start");
    let _ = terminal(&service, &run.id).await;
    assert!(
        calls.load(Ordering::SeqCst) > 0,
        "an approved paid run should reach the provider"
    );

    // Resume and rerun still honour the gate for a stored Cloud request with no approval.
    let provider = Arc::new(ReplayProvider::synthetic());
    let store = Arc::new(SqliteStore::in_memory().unwrap());
    let service = WorkflowService::new(store.clone(), provider.clone(), Handle::current());
    let p = service.create_project(intake()).await.unwrap();
    for approved in [false, true] {
        let mut req = request(
            &p,
            provider.as_ref(),
            if approved {
                "paid-approved"
            } else {
                "paid-declined"
            },
        );
        req.input.provider.kind = ProviderKind::Cloud;
        req.input.authorization.paid_provider = approved;
        assert!(service.execute(Command::Start(req.clone())).await.is_err());
        // Even an already persisted legacy/malicious request cannot bypass resume.
        store.create_run(&req).unwrap();
        assert!(
            service
                .execute(Command::Resume {
                    id: req.id.clone(),
                    authorization: Authorization {
                        synthetic_data: true,
                        ..Default::default()
                    }
                })
                .await
                .is_err()
        );
        let attempt = store.begin_attempt(&req.id, now_ms()).unwrap();
        store
            .finish_attempt(&req.id, &attempt.id, RunOutcome::Failed, None, now_ms())
            .unwrap();
        let mut rerun = req.clone();
        rerun.id = RunId::new();
        rerun.idempotency_key.push_str("-again");
        assert!(
            service
                .execute(Command::Rerun {
                    source: req.id,
                    request: rerun
                })
                .await
                .is_err()
        );
    }
    assert!(provider.requests().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_fallback_and_snapshot_mismatch_block_without_calls() {
    let provider = Arc::new(ReplayProvider::synthetic());
    let store = Arc::new(SqliteStore::in_memory().unwrap());
    let service = WorkflowService::new(store.clone(), provider.clone(), Handle::current());
    let p = service.create_project(intake()).await.unwrap();
    for mutation in 0..3 {
        let mut req = request(&p, provider.as_ref(), &format!("mismatch-{mutation}"));
        match mutation {
            0 => req.input.provider.kind = ProviderKind::Local,
            1 => req.input.prompts.sha256 = "different-prompt".into(),
            _ => req.input.provider.model = "different-model".into(),
        }
        store.create_run(&req).unwrap();
        assert!(service.start(req.clone()).await.is_err());
        assert!(
            service
                .resume(
                    req.id,
                    Authorization {
                        synthetic_data: true,
                        ..Default::default()
                    }
                )
                .await
                .is_err()
        );
    }
    assert!(provider.requests().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rerun_creates_new_version_preserves_original_and_rejects_wrong_export_identity() {
    let provider = Arc::new(ReplayProvider::synthetic().with_delay(Duration::ZERO));
    let service = WorkflowService::new(
        Arc::new(SqliteStore::in_memory().unwrap()),
        provider.clone(),
        Handle::current(),
    );
    let p = service.create_project(intake()).await.unwrap();
    let first = service
        .start(request(&p, provider.as_ref(), "v1"))
        .await
        .unwrap();
    let original = terminal(&service, &first.id).await;
    let second_request = request(&p, provider.as_ref(), "v2");
    let second = service
        .rerun(first.id.clone(), second_request)
        .await
        .unwrap();
    let rerun = terminal(&service, &second.id).await;
    assert_eq!(rerun.version, original.version + 1);
    assert_eq!(rerun.rerun_of, Some(original.id.clone()));
    assert_eq!(service.run(first.id).await.unwrap(), original);
    assert_eq!(service.history(p.id).await.unwrap().len(), 2);
    assert_eq!(provider.requests().len(), 12);
    let mut wrong = rerun.clone();
    wrong.report.as_mut().unwrap().version = original.version;
    assert!(export_html(&wrong).is_err());
    wrong = rerun.clone();
    wrong.report.as_mut().unwrap().run_id = RunId::new();
    assert!(export_markdown(&wrong).is_err());
    let mut edited = rerun;
    edited
        .report
        .as_mut()
        .unwrap()
        .markdown
        .push_str("\nSaved edited report.");
    assert!(
        export_markdown(&edited)
            .unwrap()
            .contains("Saved edited report.")
    );
}

#[test]
fn upstream_input_composition_and_prompt_disclaimers_are_retained() {
    let mut outputs = BTreeMap::new();
    outputs.insert(Stage::TechnicalRestatement, "technical restatement".into());
    let input = Intake {
        title: "Tray".into(),
        description: "Description".into(),
        ..Default::default()
    };
    assert_eq!(
        prompts::narrative(&input),
        "**Invention Title:** Tray\n\n**Description:** Description"
    );
    assert_eq!(
        prompts::user(Stage::PriorArt, &run_input(input.clone()), &outputs),
        "## Invention (Technical Restatement from Stage 1)\n\ntechnical restatement\n\n## Original Inventor Description\n\n**Invention Title:** Tray\n\n**Description:** Description"
    );
    for stage in Stage::ALL {
        let system = prompts::system(stage);
        assert!(system.contains("does not constitute legal advice"));
        assert!(system.contains("registered patent attorney"));
    }
    assert_eq!(
        prompts::snapshot(WorkflowKind::Feasibility).sha256.len(),
        64
    );
}

#[test]
fn all_nine_legacy_fields_are_trimmed_in_original_order_in_every_stage_input() {
    let input = Intake {
        title: "  Tray  ".into(),
        description: " Description\n".into(),
        context: original_context(),
        ..Default::default()
    };
    let mut expected = "**Invention Title:** Tray\n\n**Description:** Description".to_string();
    for (label, text) in LEGACY_FIELDS {
        expected.push_str(&format!("\n\n**{label}:** {text}"));
    }
    assert_eq!(prompts::narrative(&input), expected);
    for stage in Stage::ALL {
        let message = prompts::user(stage, &run_input(input.clone()), &BTreeMap::new());
        assert!(message.contains(&expected));
        assert_legacy_context(&message);
    }
    let blank = Intake {
        context: InventionContext {
            problem_solved: " \t\n".into(),
            how_it_works: " mechanism ".into(),
            additional_notes: "\n\n".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(prompts::narrative(&blank), "**How It Works:** mechanism");
}

struct FailingProvider(AtomicUsize);
#[async_trait]
impl Provider for FailingProvider {
    fn snapshot(&self) -> ProviderSnapshot {
        ReplayProvider::synthetic().snapshot()
    }
    async fn generate(
        &self,
        _: ProviderRequest,
        _: CancellationToken,
        _: UnboundedSender<String>,
    ) -> Result<ProviderResponse> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(Error::Provider(
            "SYNTHETIC_PRIVATE_RESPONSE_BODY must not be persisted".into(),
        ))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failure_notice_is_durable_sanitized_and_not_an_unsupported_event_append() {
    let provider = Arc::new(FailingProvider(AtomicUsize::new(0)));
    let service = WorkflowService::new(
        Arc::new(SqliteStore::in_memory().unwrap()),
        provider.clone(),
        Handle::current(),
    );
    let project = service.create_project(intake()).await.unwrap();
    let started = service
        .start(request(&project, provider.as_ref(), "safe-failure"))
        .await
        .unwrap();
    let failed = terminal(&service, &started.id).await;
    assert_eq!(failed.outcome, RunOutcome::Failed);
    let events = service
        .events(Subscribe {
            run_id: failed.id,
            after_sequence: 0,
        })
        .await
        .unwrap();
    assert!(events.iter().any(|event| matches!(&event.event,
        EventKind::OutcomeChanged { outcome: RunOutcome::Failed, message: Some(message) }
        if message == "Run failed. Completed stages were preserved; review settings before resuming.")));
    assert!(
        !serde_json::to_string(&events)
            .unwrap()
            .contains("SYNTHETIC_PRIVATE_RESPONSE_BODY")
    );
    assert_eq!(provider.0.load(Ordering::SeqCst), 1);
}
