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
            .and_then(|u| u.host_str().map(str::to_string))
            .is_some_and(|host| {
                host.eq_ignore_ascii_case("localhost")
                    || host == "[::1]"
                    || host == "::1"
                    || host
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback())
            })
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
        if request.input.provider != self.snapshot
            || !request.input.authorization.synthetic_data
            || !request.input.authorization.local_inference
        {
            return Err(Error::Invalid("local inference requires exact endpoint/model and explicit synthetic local authorization".into()));
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
                Some(Ok(bytes)) => pending.extend_from_slice(&bytes),
                Some(Err(_)) => {
                    return Err(Error::Provider(
                        "Ollama stream interrupted; partial stage not committed".into(),
                    ));
                }
                None if !pending.is_empty() => pending.push(b'\n'),
                None => {
                    return Err(Error::Provider(
                        "Ollama stream ended without a completion frame".into(),
                    ));
                }
            }
            while let Some(newline) = pending.iter().position(|b| *b == b'\n') {
                let line: Vec<_> = pending.drain(..=newline).collect();
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
                        if text.trim().is_empty() {
                            return Err(Error::Provider(
                                "Ollama completed with no analysis text".into(),
                            ));
                        }
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
        assert!(
            !OllamaProvider::new("http://192.0.2.1:11434", "model")
                .unwrap()
                .is_on_machine()
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
