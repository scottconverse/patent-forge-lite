//! Talk to one OpenAI-compatible server and print what came back.
//!
//! A development probe, not a test: it proves the adapter against a real runtime on this machine
//! without needing any credential.
//!
//!   cargo run -p pfl-workflow --example chat_probe -- http://127.0.0.1:1234 <model> "prompt"
use pfl_workflow::providers::OpenAiCompatibleProvider;
use pfl_workflow::*;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [base, model, prompt] = args.as_slice() else {
        return Err(Error::Invalid(
            "usage: chat_probe <base-url> <model> <prompt>".into(),
        ));
    };
    let provider = OpenAiCompatibleProvider::new(base.clone(), model.clone(), None)?;
    let request = ProviderRequest {
        run_id: RunId("probe".into()),
        attempt_id: AttemptId("probe".into()),
        stage: Stage::TechnicalRestatement,
        input: RunInput {
            kind: WorkflowKind::Feasibility,
            intake: Intake::default(),
            provider: provider.snapshot(),
            prompts: PromptSnapshot {
                version: "probe".into(),
                sha256: "0".repeat(64),
            },
            authorization: Authorization::default(),
            upstream: Upstream::default(),
            regeneration: None,
            provider_plan: Vec::new(),
        },
        system_prompt: "Answer in one short sentence.".into(),
        user_prompt: prompt.clone(),
        prior_outputs: Vec::new(),
        effort: None,
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let pump = tokio::spawn(async move {
        let mut chunks = 0usize;
        while rx.recv().await.is_some() {
            chunks += 1;
        }
        chunks
    });
    let started = std::time::Instant::now();
    let response = provider
        .generate(request, tokio_util::sync::CancellationToken::new(), tx)
        .await?;
    let chunks = pump.await.unwrap_or(0);
    println!("base:     {base}");
    println!("model:    {model}");
    println!("answer:   {}", response.text.trim());
    println!("deltas:   {chunks}");
    println!(
        "usage:    input={:?} output={:?}",
        response.usage.input_tokens, response.usage.output_tokens
    );
    println!("elapsed:  {:?}", started.elapsed());
    Ok(())
}
