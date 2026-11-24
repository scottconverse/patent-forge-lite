use crate::*;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Mutex, time::Duration};
use tokio::sync::mpsc::UnboundedSender;

/// Synthetic recorded responses. This never contacts a model or research API.
pub struct ReplayProvider {
    outputs: BTreeMap<Stage, String>,
    delay: Duration,
    requests: Mutex<Vec<ProviderRequest>>,
}

impl ReplayProvider {
    pub fn synthetic() -> Self {
        let records: Vec<String> = serde_json::from_str(include_str!(
            "../../../fixtures/providers/synthetic-feasibility.json"
        ))
        .expect("bundled synthetic records must be valid JSON");
        Self::new(Stage::ALL.into_iter().zip(records).collect())
    }

    /// Deterministic synthetic responses for every stage of every workflow.
    /// Never a model output and never real invention data.
    pub fn all_workflows() -> Self {
        Self::new(
            Stage::EVERY_STAGE
                .into_iter()
                .map(|stage| {
                    (
                        stage,
                        format!(
                            "SYNTHETIC RECORDED OUTPUT — {} ({}) — not a model response.",
                            stage.label(),
                            stage.key()
                        ),
                    )
                })
                .collect(),
        )
    }
    pub fn new(outputs: BTreeMap<Stage, String>) -> Self {
        Self {
            outputs,
            delay: Duration::from_millis(2),
            requests: Mutex::new(Vec::new()),
        }
    }
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
    /// Synthetic acceptance evidence, never a log of real invention data.
    pub fn requests(&self) -> Vec<ProviderRequest> {
        self.requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn request_count(&self) -> usize {
        self.requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }
    pub fn stage_calls(&self) -> BTreeMap<Stage, usize> {
        let mut calls = BTreeMap::new();
        for request in self
            .requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            *calls.entry(request.stage).or_insert(0) += 1;
        }
        calls
    }
}

#[async_trait]
impl Provider for ReplayProvider {
    fn snapshot(&self) -> ProviderSnapshot {
        ProviderSnapshot {
            kind: ProviderKind::Replay,
            model: "synthetic-feasibility-v1".into(),
            endpoint: None,
        }
    }
    async fn generate(
        &self,
        request: ProviderRequest,
        cancellation: CancellationToken,
        deltas: UnboundedSender<String>,
    ) -> Result<ProviderResponse> {
        self.requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(request.clone());
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if request.input.provider != self.snapshot() || !request.input.authorization.synthetic_data
        {
            return Err(Error::Invalid(
                "recorded provider requires its exact snapshot and synthetic input authorization"
                    .into(),
            ));
        }
        let text = self
            .outputs
            .get(&request.stage)
            .cloned()
            .ok_or_else(|| Error::Provider("recorded stage missing".into()))?;
        let chars: Vec<char> = text.chars().collect();
        for chunk in chars.chunks(32) {
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => return Err(Error::Cancelled),
                _ = tokio::time::sleep(self.delay) => {}
            }
            if deltas.send(chunk.iter().collect()).is_err() {
                return Err(Error::Cancelled);
            }
        }
        Ok(ProviderResponse {
            text,
            usage: Usage::default(),
        })
    }
}

/// Anthropic Messages API over SSE. Fail-closed: construction alone never
/// contacts the network, and every call additionally requires an explicit paid
/// authorization flag on the run input.
pub struct CloudProvider {
    snapshot: ProviderSnapshot,
    client: reqwest::Client,
    api_key: String,
    endpoint: String,
}

impl CloudProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Result<Self> {
        Self::with_endpoint(api_key, model, "https://api.anthropic.com")
    }

    /// Endpoint override exists for deterministic local fixtures and never
    /// silently redirects a configured default.
    pub fn with_endpoint(
        api_key: impl Into<String>,
        model: impl Into<String>,
        endpoint: impl Into<String>,
    ) -> Result<Self> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(Error::Invalid("cloud provider requires an API key".into()));
        }
        let model = model.into();
        if model.trim().is_empty() {
            return Err(Error::Invalid("cloud provider requires a model".into()));
        }
        let endpoint = endpoint.into().trim_end_matches('/').to_string();
        let url = reqwest::Url::parse(&endpoint)
            .map_err(|_| Error::Invalid("invalid cloud endpoint URL".into()))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || url.username() != ""
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::Invalid(
                "cloud endpoint must be HTTP(S), with a host and no credentials, query or fragment"
                    .into(),
            ));
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(1800))
            .build()
            .map_err(|_| Error::Provider("cannot initialize cloud HTTP client".into()))?;
        Ok(Self {
            snapshot: ProviderSnapshot {
                kind: ProviderKind::Cloud,
                // The key is never part of a snapshot.
                endpoint: Some(endpoint.clone()),
                model,
            },
            client,
            api_key,
            endpoint,
        })
    }
}

#[async_trait]
impl Provider for CloudProvider {
    fn snapshot(&self) -> ProviderSnapshot {
        self.snapshot.clone()
    }
    async fn generate(
        &self,
        request: ProviderRequest,
        cancellation: CancellationToken,
        deltas: UnboundedSender<String>,
    ) -> Result<ProviderResponse> {
        if request.input.provider != self.snapshot || !request.input.authorization.paid_provider {
            return Err(Error::Invalid(
                "cloud inference requires the exact endpoint/model and explicit paid authorization"
                    .into(),
            ));
        }
        if request.input.authorization.synthetic_data == request.input.authorization.real_data {
            return Err(Error::Invalid(
                "the run must carry exactly one data declaration: synthetic test material or a real invention"
                    .into(),
            ));
        }
        // Real-invention data stays Local-only until Cloud disclosure and consent
        // are designed and approved. Deliberately narrower than the local adapter.
        if request.input.authorization.real_data {
            return Err(Error::Invalid(
                "real-invention data is local-only in this build; cloud runs require synthetic test data"
                    .into(),
            ));
        }
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let send = self
            .client
            .post(format!("{}/v1/messages", self.endpoint))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({
                "model": self.snapshot.model,
                "max_tokens": CLOUD_MAX_TOKENS,
                "stream": true,
                "system": request.system_prompt,
                "messages": [{ "role": "user", "content": request.user_prompt }]
            }))
            .send();
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(Error::Cancelled),
            r = send => r.map_err(|_| Error::Provider("cloud request failed; no alternate destination attempted".into()))?
        };
        if !response.status().is_success() {
            // A status the operator can act on gets its own sentence: a bad key, a rate limit
            // and an outage need different responses.
            let status = response.status().as_u16();
            let advice = match status {
                401 | 403 => "the key was rejected; check the session key",
                429 => "the provider is rate-limiting this account; wait and try again",
                500..=599 => "the provider is failing; try again later",
                _ => "the provider refused the request",
            };
            return Err(Error::Provider(format!("cloud HTTP {status}: {advice}")));
        }
        let mut stream = response.bytes_stream();
        let mut pending: Vec<u8> = Vec::new();
        let mut text = String::new();
        let mut input_tokens = None;
        let mut output_tokens = None;
        let mut complete = false;
        let mut stop_reason: Option<String> = None;
        loop {
            let item = tokio::select! {
                biased;
                _ = cancellation.cancelled() => return Err(Error::Cancelled),
                item = stream.next() => item
            };
            match item {
                Some(Ok(bytes)) => {
                    pending.extend_from_slice(&bytes);
                    if pending.len() > CLOUD_MAX_FRAME_BYTES {
                        return Err(Error::Incomplete(
                            "cloud stream exceeded the frame buffer limit; partial text was not committed".into(),
                        ));
                    }
                }
                Some(Err(_)) => {
                    return Err(Error::Provider(
                        "cloud stream interrupted; partial stage not committed".into(),
                    ));
                }
                None => {
                    // Bytes left in the buffer without a newline mean the stream was
                    // cut mid-frame; nothing after this can be trusted.
                    if !pending.iter().all(u8::is_ascii_whitespace) {
                        return Err(Error::Incomplete(
                            "cloud stream ended mid-frame; partial text was not committed".into(),
                        ));
                    }
                    break;
                }
            }
            while let Some(raw) = take_sse_line(&mut pending) {
                let Ok(line) = String::from_utf8(raw) else {
                    return Err(Error::Provider(
                        "cloud stream contained invalid UTF-8; the stage is not complete".into(),
                    ));
                };
                let line = line.trim_end();
                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let data = data.trim();
                if data == "[DONE]" {
                    continue;
                }
                // A malformed frame is a protocol fault, not a frame to skip: silently
                // continuing could drop the text or the stop reason that proves the
                // stage finished.
                let Ok(frame) = serde_json::from_str::<serde_json::Value>(data) else {
                    return Err(Error::Provider(
                        "cloud stream contained a malformed frame; the stage is not complete"
                            .into(),
                    ));
                };
                match frame["type"].as_str() {
                    Some("content_block_delta") => {
                        if let Some(chunk) = frame["delta"]["text"].as_str() {
                            text.push_str(chunk);
                            if deltas.send(chunk.to_string()).is_err() {
                                return Err(Error::Cancelled);
                            }
                        }
                    }
                    Some("message_start") => {
                        input_tokens = frame["message"]["usage"]["input_tokens"].as_u64();
                    }
                    Some("message_delta") => {
                        output_tokens = frame["usage"]["output_tokens"].as_u64();
                        if let Some(reason) = frame["delta"]["stop_reason"].as_str() {
                            stop_reason = Some(reason.to_string());
                        }
                    }
                    Some("message_stop") => complete = true,
                    Some("error") => {
                        // The provider names the fault in `error.type`; dropping it leaves
                        // the operator with "an error" and nothing to act on.
                        let kind = frame["error"]["type"]
                            .as_str()
                            .map(|kind| format!(" ({kind})"))
                            .unwrap_or_default();
                        return Err(Error::Provider(format!(
                            "cloud provider reported an error{kind}"
                        )));
                    }
                    _ => {}
                }
            }
        }
        require_terminal_frame(
            complete,
            &text,
            "cloud",
            stop_reason.as_deref(),
            CLOUD_COMPLETE_STOPS,
        )?;
        Ok(ProviderResponse {
            text,
            usage: Usage {
                input_tokens,
                output_tokens,
            },
        })
    }
}

/// Loopback means the endpoint resolves to this machine: the literal names, or a
/// loopback IP. Everything else is a remote runtime and is refused for local mode.
fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host == "[::1]"
        || host == "::1"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Removes one complete SSE line from the pending byte buffer, newline included.
/// Buffering bytes rather than decoded text is what keeps a multi-byte character
/// split across two chunks intact instead of becoming U+FFFD.
fn take_sse_line(pending: &mut Vec<u8>) -> Option<Vec<u8>> {
    let index = pending.iter().position(|byte| *byte == b'\n')?;
    Some(pending.drain(..=index).collect())
}

/// Removes one complete NDJSON line from the pending byte buffer, newline included.
/// Bytes are accumulated before decoding so a multi-byte character split across
/// network chunks is reassembled rather than mangled.
fn take_ndjson_line(pending: &mut Vec<u8>) -> Option<Vec<u8>> {
    let index = pending.iter().position(|byte| *byte == b'\n')?;
    Some(pending.drain(..=index).collect())
}

/// The single rule for "did this provider actually finish": a terminal frame was
/// seen and the text is not blank. Anything else is incomplete, never Completed.
/// Which stop reasons prove a stage finished. Anything else — an output limit, a
/// continuation state such as `tool_use`, or no reason at all — is incomplete.
const OLLAMA_COMPLETE_STOPS: &[&str] = &["stop"];
const CLOUD_COMPLETE_STOPS: &[&str] = &["end_turn", "stop_sequence"];

/// Largest partial frame the Cloud stream may hold. A stream that never sends a newline
/// would otherwise grow this buffer without limit.
const CLOUD_MAX_FRAME_BYTES: usize = 2 * 1024 * 1024;

/// Output ceiling for one Cloud turn.
///
/// Chosen from the prompts, not from a guess: the largest target is the feasibility report's
/// "3000-5000 words TOTAL" (`prompts/feasibility/stage-6.md`), which is roughly 6,700 tokens
/// of prose before tables and headings. 16,000 leaves room above that while staying far
/// inside the model's own output limit. A cap below the prompt target is not a saving: the
/// stage stops at the cap, is correctly treated as incomplete, and the paid run is wasted.
pub const CLOUD_MAX_TOKENS: u32 = 16_000;

fn require_terminal_frame(
    complete: bool,
    text: &str,
    provider: &str,
    stop_reason: Option<&str>,
    accepted_stops: &[&str],
) -> Result<()> {
    if !complete {
        return Err(Error::Incomplete(format!(
            "{provider} stream ended before its completion frame; partial text was not committed"
        )));
    }
    // A terminal frame is not the same as a finished answer. A successful stop
    // reason must be present and recognised; everything else is unsupported.
    let Some(reason) = stop_reason else {
        return Err(Error::Incomplete(format!(
            "{provider} reported no stop reason, so the stage cannot be proven finished"
        )));
    };
    if !accepted_stops.contains(&reason) {
        // An output-limit stop is the one refusal the operator can act on, so it says so
        // rather than borrowing the generic wording.
        if reason == "max_tokens" {
            return Err(Error::Incomplete(format!(
                "{provider} stopped at its output limit (max_tokens); the stage is not complete and nothing was saved"
            )));
        }
        return Err(Error::Incomplete(format!(
            "{provider} stopped with unsupported reason '{reason}'; the stage is not complete"
        )));
    }
    if text.trim().is_empty() {
        return Err(Error::Provider(format!(
            "{provider} completed with no analysis text"
        )));
    }
    Ok(())
}

/// Direct Ollama NDJSON streaming. Construction validates configuration only,
/// and does not inspect an installation, contact an endpoint, or acquire a model.
pub struct OllamaProvider {
    snapshot: ProviderSnapshot,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Result<Self> {
        let endpoint = endpoint.into().trim_end_matches('/').to_string();
        let url = reqwest::Url::parse(&endpoint)
            .map_err(|_| Error::Invalid("invalid Ollama endpoint URL".into()))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::Invalid("Ollama endpoint must be HTTP(S), with a host and no credentials, query or fragment".into()));
        }
        if !is_loopback_host(url.host_str().unwrap_or("")) {
            return Err(Error::Invalid(
                "on-machine inference requires a loopback endpoint (127.0.0.1, ::1 or localhost); remote runtimes are not supported in this build".into(),
            ));
        }
        let model = model.into();
        if model.trim().is_empty() {
            return Err(Error::Invalid("Ollama model is required".into()));
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(1800))
            .build()
            .map_err(|_| Error::Provider("cannot initialize Ollama HTTP client".into()))?;
        Ok(Self {
            snapshot: ProviderSnapshot {
                kind: ProviderKind::Local,
                endpoint: Some(endpoint),
                model,
            },
            client,
        })
    }
    pub fn is_on_machine(&self) -> bool {
        reqwest::Url::parse(self.snapshot.endpoint.as_deref().unwrap_or(""))
            .ok()
            .and_then(|u| u.host_str().map(is_loopback_host))
            .unwrap_or(false)
    }
}

#[derive(Deserialize)]
struct OllamaMessage {
    #[serde(default)]
    content: String,
}
#[derive(Deserialize)]
struct OllamaFrame {
    message: Option<OllamaMessage>,
    #[serde(default)]
    done: bool,
    /// Why generation stopped: `stop` for a finished answer, `length` when the
    /// model hit its output limit.
    #[serde(default)]
    done_reason: Option<String>,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
    error: Option<String>,
}

fn decode_line(line: &[u8]) -> Result<Option<OllamaFrame>> {
    if line.iter().all(u8::is_ascii_whitespace) {
        return Ok(None);
    }
    serde_json::from_slice(line)
        .map(Some)
        .map_err(|_| Error::Provider("invalid Ollama streaming JSON".into()))
}

#[async_trait]
impl Provider for OllamaProvider {
    fn snapshot(&self) -> ProviderSnapshot {
        self.snapshot.clone()
    }
    async fn generate(
        &self,
        request: ProviderRequest,
        cancellation: CancellationToken,
        deltas: UnboundedSender<String>,
    ) -> Result<ProviderResponse> {
        if request.input.provider != self.snapshot || !request.input.authorization.local_inference {
            return Err(Error::Invalid(
                "local inference requires the exact endpoint/model and explicit local authorization"
                    .into(),
            ));
        }
        if request.input.authorization.synthetic_data == request.input.authorization.real_data {
            return Err(Error::Invalid(
                "the run must carry exactly one data declaration: synthetic test material or a real invention"
                    .into(),
            ));
        }
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let endpoint = format!(
            "{}/api/chat",
            self.snapshot.endpoint.as_deref().unwrap_or("")
        );
        let send = self
            .client
            .post(endpoint)
            .json(&serde_json::json!({
                "model": self.snapshot.model, "stream": true,
                "messages": [
                    { "role": "system", "content": request.system_prompt },
                    { "role": "user", "content": request.user_prompt }
                ],
                "options": { "num_predict": 32000 }
            }))
            .send();
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(Error::Cancelled),
            r = send => r.map_err(|_| Error::Provider("Ollama request failed; no alternate destination attempted".into()))?
        };
        if !response.status().is_success() {
            return Err(Error::Provider(format!(
                "Ollama HTTP {}; no fallback attempted",
                response.status().as_u16()
            )));
        }
        let mut stream = response.bytes_stream();
        let mut pending = Vec::new();
        let mut text = String::new();
        loop {
            let item = tokio::select! {
                biased;
                _ = cancellation.cancelled() => return Err(Error::Cancelled),
                item = stream.next() => item
            };
            match item {
                Some(Ok(bytes)) => {
                    pending.extend_from_slice(&bytes);
                    if pending.len() > CLOUD_MAX_FRAME_BYTES {
                        return Err(Error::Incomplete(
                            "cloud stream exceeded the frame buffer limit; partial text was not committed".into(),
                        ));
                    }
                }
                Some(Err(_)) => {
                    return Err(Error::Provider(
                        "Ollama stream interrupted; partial stage not committed".into(),
                    ));
                }
                None if !pending.is_empty() => pending.push(b'\n'),
                None => {
                    return Err(Error::Incomplete(
                        "Ollama stream ended before its completion frame; partial text was not committed".into(),
                    ));
                }
            }
            while let Some(line) = take_ndjson_line(&mut pending) {
                if let Some(frame) = decode_line(&line)? {
                    if frame.error.is_some() {
                        return Err(Error::Provider("Ollama reported an inference error".into()));
                    }
                    if let Some(message) = frame.message {
                        text.push_str(&message.content);
                        if !message.content.is_empty() && deltas.send(message.content).is_err() {
                            return Err(Error::Cancelled);
                        }
                    }
                    if frame.done {
                        if cancellation.is_cancelled() {
                            return Err(Error::Cancelled);
                        }
                        require_terminal_frame(
                            true,
                            &text,
                            "Ollama",
                            frame.done_reason.as_deref(),
                            OLLAMA_COMPLETE_STOPS,
                        )?;
                        return Ok(ProviderResponse {
                            text,
                            usage: Usage {
                                input_tokens: frame.prompt_eval_count,
                                output_tokens: frame.eval_count,
                            },
                        });
                    }
                }
            }
            if pending.len() > 2 * 1024 * 1024 {
                return Err(Error::Provider(
                    "Ollama streaming frame exceeds 2 MiB".into(),
                ));
            }
        }
    }
}

/// Lists the models an on-machine Ollama already has, so setup can offer a choice
/// instead of demanding a typed identifier.
///
/// Loopback only, read-only, and no inference: `GET /api/tags` is a catalogue read.
pub async fn list_local_models(endpoint: &str) -> Result<Vec<String>> {
    let endpoint = endpoint.trim_end_matches('/');
    let url = reqwest::Url::parse(endpoint)
        .map_err(|_| Error::Invalid("invalid Ollama endpoint URL".into()))?;
    if !is_loopback_host(url.host_str().unwrap_or("")) {
        return Err(Error::Invalid(
            "listing models requires a loopback endpoint; remote runtimes are not supported in this build".into(),
        ));
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| Error::Provider("cannot initialize the Ollama HTTP client".into()))?;
    let response = client
        .get(format!("{endpoint}/api/tags"))
        .send()
        .await
        .map_err(|_| {
            Error::Provider("Ollama is not reachable on the configured endpoint".into())
        })?;
    if !response.status().is_success() {
        return Err(Error::Provider(format!(
            "Ollama returned HTTP {} for its model list",
            response.status().as_u16()
        )));
    }
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|_| Error::Provider("Ollama returned a malformed model list".into()))?;
    let mut names: Vec<String> = body["models"]
        .as_array()
        .map(|models| {
            models
                .iter()
                .filter_map(|model| model["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names.dedup();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_validation_has_no_network_side_effects() {
        assert!(OllamaProvider::new("http://user:secret@localhost:11434", "model").is_err());
        assert!(OllamaProvider::new("http://localhost:11434?secret=x", "model").is_err());
        assert!(OllamaProvider::new("file:///tmp/model", "model").is_err());
        assert!(
            OllamaProvider::new("http://127.0.0.1:11434", "model")
                .unwrap()
                .is_on_machine()
        );
        // A remote runtime cannot be represented as on-machine inference.
        assert!(OllamaProvider::new("http://192.0.2.1:11434", "model").is_err());
        assert!(OllamaProvider::new("https://ollama.example.com", "model").is_err());
        assert!(OllamaProvider::new("http://[::1]:11434", "model").is_ok());
        assert!(OllamaProvider::new("http://localhost:11434", "model").is_ok());
    }

    #[test]
    fn the_old_per_chunk_decode_corrupted_split_characters() {
        // Documents the defect the byte buffer fixes: decoding each network chunk
        // independently turns a character split across chunks into U+FFFD.
        let first = b"data: {\"text\":\"caf\xc3";
        let second = b"\xa9\"}\n";
        let lossy = format!(
            "{}{}",
            String::from_utf8_lossy(first),
            String::from_utf8_lossy(second)
        );
        assert!(
            lossy.contains('\u{FFFD}'),
            "per-chunk lossy decoding is expected to corrupt the split character"
        );
    }

    #[test]
    fn split_utf8_across_sse_chunks_reassembles_intact() {
        let mut pending: Vec<u8> = Vec::new();
        pending.extend_from_slice(b"data: {\"text\":\"caf\xc3");
        assert!(
            take_sse_line(&mut pending).is_none(),
            "no complete line yet"
        );
        pending.extend_from_slice(b"\xa9 \xf0\x9f\xa7\xaa\"}\n");
        let raw = take_sse_line(&mut pending).expect("complete line");
        let line = String::from_utf8(raw).expect("valid utf-8 after reassembly");
        assert!(line.contains("café 🧪"), "got {line}");
        assert!(pending.is_empty());
    }

    #[test]
    fn an_invalid_utf8_line_is_rejected_rather_than_replaced() {
        let mut pending: Vec<u8> = Vec::new();
        pending.extend_from_slice(b"data: \xff\xfe\n");
        let raw = take_sse_line(&mut pending).expect("complete line");
        assert!(
            String::from_utf8(raw).is_err(),
            "invalid UTF-8 must be an error, not U+FFFD"
        );
    }

    #[test]
    fn fragmented_network_chunks_reassemble_without_mangling_unicode() {
        // The same line arrives in two chunks, with the multi-byte character split
        // across the boundary.
        let mut pending = Vec::new();
        pending.extend_from_slice(b"{\"message\":{\"content\":\"caf\xc3");
        assert!(take_ndjson_line(&mut pending).is_none());
        pending.extend_from_slice(b"\xa9 \xf0\x9f\xa7\xaa\"},\"done\":true}\n");
        let line = take_ndjson_line(&mut pending).expect("complete line");
        let text = String::from_utf8(line).expect("utf8");
        assert!(text.contains("café 🧪"));
        assert!(pending.is_empty());
    }

    #[test]
    fn a_stream_without_its_terminal_frame_is_incomplete_not_completed() {
        let error = require_terminal_frame(
            false,
            "partial analysis",
            "Ollama",
            None,
            OLLAMA_COMPLETE_STOPS,
        )
        .unwrap_err();
        assert!(matches!(error, Error::Incomplete(_)));
        // Accepted stop but no text is its own provider error.
        let empty =
            require_terminal_frame(true, "   ", "cloud", Some("end_turn"), CLOUD_COMPLETE_STOPS)
                .unwrap_err();
        assert!(matches!(empty, Error::Provider(_)));
        assert!(
            require_terminal_frame(
                true,
                "complete analysis",
                "Ollama",
                Some("stop"),
                OLLAMA_COMPLETE_STOPS
            )
            .is_ok()
        );
    }

    #[test]
    fn only_a_successful_stop_reason_completes_a_stage() {
        for reason in [
            "length",
            "max_tokens",
            "max_output_tokens",
            "tool_use",
            "pause_turn",
            "something_new",
        ] {
            let error = require_terminal_frame(
                true,
                "whole answer so far",
                "cloud",
                Some(reason),
                CLOUD_COMPLETE_STOPS,
            )
            .unwrap_err();
            assert!(
                matches!(error, Error::Incomplete(_)),
                "{reason} must be incomplete, got {error:?}"
            );
        }
        // A limit is named in the message so the operator can act on it.
        let limit = require_terminal_frame(
            true,
            "text",
            "Ollama",
            Some("length"),
            OLLAMA_COMPLETE_STOPS,
        )
        .unwrap_err();
        assert!(limit.to_string().contains("unsupported reason 'length'"));
        // A Cloud output-limit stop names the limit, because raising the cap is the fix.
        let cloud_limit = require_terminal_frame(
            true,
            "text",
            "cloud",
            Some("max_tokens"),
            CLOUD_COMPLETE_STOPS,
        )
        .unwrap_err();
        assert!(
            cloud_limit.to_string().contains("output limit"),
            "a max_tokens stop should name the limit, got {cloud_limit}"
        );
        assert!(matches!(cloud_limit, Error::Incomplete(_)));
        // The cap must clear the largest prompt target: stage-6 asks for up to 5000 words,
        // which is about 6670 tokens of prose before headings and tables.
        let largest_target_words: usize = 5_000;
        let words_to_tokens: usize = (largest_target_words * 4) / 3;
        let required = words_to_tokens + 2_000;
        assert!(
            CLOUD_MAX_TOKENS as usize >= required,
            "the Cloud cap ({CLOUD_MAX_TOKENS}) must clear the largest prompt target ({required} tokens)"
        );
        // No reason at all cannot prove the stage finished.
        let missing =
            require_terminal_frame(true, "text", "cloud", None, CLOUD_COMPLETE_STOPS).unwrap_err();
        assert!(missing.to_string().contains("no stop reason"));
        // The accepted set differs per provider.
        for reason in OLLAMA_COMPLETE_STOPS {
            assert!(
                require_terminal_frame(true, "text", "Ollama", Some(reason), OLLAMA_COMPLETE_STOPS)
                    .is_ok()
            );
        }
        for reason in CLOUD_COMPLETE_STOPS {
            assert!(
                require_terminal_frame(true, "text", "cloud", Some(reason), CLOUD_COMPLETE_STOPS)
                    .is_ok()
            );
        }
        assert!(
            require_terminal_frame(true, "text", "cloud", Some("stop"), CLOUD_COMPLETE_STOPS)
                .is_err()
        );
    }
    #[test]
    fn streaming_frames_preserve_unicode_and_unknown_usage() {
        let frame =
            decode_line("{\"message\":{\"content\":\"café 🧪\"},\"done\":true}\n".as_bytes())
                .unwrap()
                .unwrap();
        assert_eq!(frame.message.unwrap().content, "café 🧪");
        assert!(frame.prompt_eval_count.is_none());
        assert!(decode_line(b"not-json").is_err());
    }
}
