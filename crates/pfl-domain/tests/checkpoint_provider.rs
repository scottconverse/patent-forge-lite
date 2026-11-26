//! The checkpoint's provider field: the new shape round-trips, and an older one still reads.
use pfl_domain::*;

fn sample(provider: Option<ProviderSnapshot>) -> Checkpoint {
    Checkpoint {
        stage: Stage::TechnicalRestatement,
        attempt_id: AttemptId("a".into()),
        text: "answer".into(),
        usage: Usage::default(),
        completed_at_ms: 7,
        provider,
    }
}

#[test]
fn a_checkpoint_without_a_provider_still_reads() {
    // The old shape, derived from a real one: serialize, then drop the field.
    let mut value = serde_json::to_value(sample(Some(ProviderSnapshot {
        kind: ProviderKind::ClaudeCode,
        model: "m".into(),
        endpoint: None,
    })))
    .unwrap();
    value.as_object_mut().expect("an object").remove("provider");
    let parsed: Checkpoint = serde_json::from_value(value).expect("an older checkpoint reads");
    assert_eq!(parsed.provider, None);
    assert_eq!(parsed.text, "answer");
}

#[test]
fn a_checkpoint_keeps_the_provider_that_answered() {
    let checkpoint = sample(Some(ProviderSnapshot {
        kind: ProviderKind::ClaudeCode,
        model: "claude-sonnet-4-5".into(),
        endpoint: None,
    }));
    let round_trip: Checkpoint =
        serde_json::from_str(&serde_json::to_string(&checkpoint).unwrap()).unwrap();
    assert_eq!(round_trip, checkpoint);
}
