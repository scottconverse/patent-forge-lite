//! What local model servers are running on this machine, and what they have loaded.
//!
//! The distinction that matters is **loaded** versus merely downloaded: LM Studio's `/v1/models`
//! lists everything on disk, so "the server answered" is not "a model is ready to answer". The
//! application never loads or unloads a model to find out — it reports what is loaded and skips
//! what is not.

use std::{collections::BTreeSet, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalKind {
    LmStudio,
    Ollama,
    LlamaCpp,
}

impl LocalKind {
    pub fn slug(self) -> &'static str {
        match self {
            LocalKind::LmStudio => "lmstudio",
            LocalKind::Ollama => "ollama",
            LocalKind::LlamaCpp => "llamacpp",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LocalKind::LmStudio => "LM Studio",
            LocalKind::Ollama => "Ollama",
            LocalKind::LlamaCpp => "llama.cpp",
        }
    }
}

/// The addresses this application looks at, and the server each one usually is.
///
/// One list, read by discovery and by the Models page: two lists that drifted would let the page
/// claim nothing answered at an address nothing probed.
pub const DISCOVERED_LOCAL_ADDRESSES: &[(&str, LocalKind)] = &[
    ("http://127.0.0.1:1234", LocalKind::LmStudio),
    ("http://127.0.0.1:11434", LocalKind::Ollama),
    ("http://127.0.0.1:8080", LocalKind::LlamaCpp),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalModel {
    pub id: String,
    /// `Some(true)` loaded, `Some(false)` downloaded only, `None` when the server does not say.
    pub loaded: Option<bool>,
    /// `llm`, `vlm`, `embeddings` or whatever the server reported.
    pub kind: String,
}

impl LocalModel {
    /// A model that can write: not an embedding model.
    pub fn writes_text(&self) -> bool {
        !self.kind.to_lowercase().contains("embed")
    }
}

#[derive(Debug, Clone)]
pub struct LocalServer {
    pub base_url: String,
    pub kind: LocalKind,
    pub models: Vec<LocalModel>,
}

#[derive(Debug, Clone, Default)]
pub struct LocalCatalog {
    pub servers: Vec<LocalServer>,
}

impl LocalCatalog {
    /// Whichever loaded writing model sorts first, deterministically.
    ///
    /// "Use whatever is loaded" is this answer and nothing else: the lowest id among loaded,
    /// non-embedding models, so the same state always yields the same choice.
    pub fn pick_loaded(&self) -> Option<(&LocalServer, &LocalModel)> {
        let mut best: Option<(&LocalServer, &LocalModel)> = None;
        for server in &self.servers {
            for model in &server.models {
                if model.loaded == Some(true)
                    && model.writes_text()
                    && best
                        .as_ref()
                        .is_none_or(|(_, current)| model.id < current.id)
                {
                    best = Some((server, model));
                }
            }
        }
        best
    }
    pub fn loaded_count(&self) -> usize {
        self.servers
            .iter()
            .flat_map(|server| &server.models)
            .filter(|model| model.loaded == Some(true))
            .count()
    }
}

/// Probes every known address. A server that does not answer is simply absent.
pub async fn discover() -> LocalCatalog {
    let mut servers = Vec::new();
    for (base, kind) in DISCOVERED_LOCAL_ADDRESSES {
        if let Some(models) = probe(base, *kind).await {
            servers.push(LocalServer {
                base_url: (*base).to_string(),
                kind: *kind,
                models,
            });
        }
    }
    LocalCatalog { servers }
}

/// Probes one address as one kind. `None` means nothing answered there in a way we understand.
pub async fn probe(base: &str, kind: LocalKind) -> Option<Vec<LocalModel>> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(4))
        .build()
        .ok()?;
    match kind {
        LocalKind::LmStudio => probe_lmstudio(&client, base).await,
        LocalKind::Ollama => probe_ollama(&client, base).await,
        LocalKind::LlamaCpp => probe_openai(&client, base).await,
    }
}

async fn get_json(client: &reqwest::Client, url: String) -> Option<serde_json::Value> {
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json::<serde_json::Value>().await.ok()
}

/// LM Studio: `/api/v0/models` says which models are loaded and what each one is for.
async fn probe_lmstudio(client: &reqwest::Client, base: &str) -> Option<Vec<LocalModel>> {
    if let Some(value) = get_json(client, format!("{base}/api/v0/models")).await {
        let models = value["data"]
            .as_array()?
            .iter()
            .filter_map(|entry| {
                Some(LocalModel {
                    id: entry["id"].as_str()?.to_string(),
                    loaded: Some(entry["state"].as_str()? == "loaded"),
                    kind: entry["type"].as_str().unwrap_or("llm").to_string(),
                })
            })
            .collect();
        return Some(models);
    }
    // Older LM Studio builds only have the OpenAI catalogue; they cannot say what is loaded.
    probe_openai(client, base).await
}

/// Ollama: `/api/tags` is everything installed, `/api/ps` is what is in memory.
async fn probe_ollama(client: &reqwest::Client, base: &str) -> Option<Vec<LocalModel>> {
    let tags = get_json(client, format!("{base}/api/tags")).await?;
    let loaded: BTreeSet<String> = match get_json(client, format!("{base}/api/ps")).await {
        Some(value) => value["models"]
            .as_array()
            .map(|models| {
                models
                    .iter()
                    .filter_map(|model| model["name"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        None => BTreeSet::new(),
    };
    Some(
        tags["models"]
            .as_array()?
            .iter()
            .filter_map(|model| {
                let id = model["name"].as_str()?.to_string();
                Some(LocalModel {
                    loaded: Some(loaded.contains(&id)),
                    kind: if model["details"]["family"]
                        .as_str()
                        .is_some_and(|family| family.to_lowercase().contains("embed"))
                    {
                        "embeddings".to_string()
                    } else {
                        "llm".to_string()
                    },
                    id,
                })
            })
            .collect(),
    )
}

/// A plain OpenAI-compatible server (llama.cpp, vLLM): one catalogue, no loaded flag.
async fn probe_openai(client: &reqwest::Client, base: &str) -> Option<Vec<LocalModel>> {
    let value = get_json(client, format!("{base}/v1/models")).await?;
    Some(
        value["data"]
            .as_array()?
            .iter()
            .filter_map(|entry| {
                Some(LocalModel {
                    id: entry["id"].as_str()?.to_string(),
                    loaded: None,
                    kind: "llm".to_string(),
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, http::StatusCode, routing::get};
    use tokio::net::TcpListener;

    async fn serve(routes: impl FnOnce(Router) -> Router) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = routes(Router::new());
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn lmstudio_reports_what_is_loaded() {
        let base = serve(|router| {
            router.route(
                "/api/v0/models",
                get(|| async {
                    (
                        StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        r#"{"data":[
                            {"id":"b-model","state":"loaded","type":"llm"},
                            {"id":"a-model","state":"not-loaded","type":"llm"},
                            {"id":"embed","state":"loaded","type":"embeddings"}
                        ]}"#,
                    )
                }),
            )
        })
        .await;
        let models = probe(&base, LocalKind::LmStudio).await.unwrap();
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].loaded, Some(true));
        assert_eq!(models[1].loaded, Some(false));
        assert!(
            !models[2].writes_text(),
            "an embedding model is not a writer"
        );
    }

    #[tokio::test]
    async fn ollama_separates_installed_from_loaded() {
        let base = serve(|router| {
            router
                .route(
                    "/api/tags",
                    get(|| async {
                        (
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            r#"{"models":[{"name":"running:8b"},{"name":"idle:8b"}]}"#,
                        )
                    }),
                )
                .route(
                    "/api/ps",
                    get(|| async {
                        (
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            r#"{"models":[{"name":"running:8b"}]}"#,
                        )
                    }),
                )
        })
        .await;
        let models = probe(&base, LocalKind::Ollama).await.unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].loaded, Some(true), "the running one is loaded");
        assert_eq!(models[1].loaded, Some(false), "the idle one is not");
    }

    #[tokio::test]
    async fn a_plain_openai_server_cannot_say_what_is_loaded() {
        let base = serve(|router| {
            router.route(
                "/v1/models",
                get(|| async {
                    (
                        StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        r#"{"data":[{"id":"only-model"}]}"#,
                    )
                }),
            )
        })
        .await;
        let models = probe(&base, LocalKind::LlamaCpp).await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].loaded, None, "the server did not say");
    }

    #[tokio::test]
    async fn nothing_answering_is_absent_not_an_error() {
        let catalog = discover().await;
        // The fixed ports may or may not be running in CI; what must hold is that probing a dead
        // address reports nothing rather than failing.
        let dead = probe("http://127.0.0.1:9", LocalKind::Ollama).await;
        assert!(dead.is_none());
        assert!(catalog.loaded_count() <= catalog.servers.iter().map(|s| s.models.len()).sum());
    }

    #[test]
    fn the_loaded_choice_is_lowest_id_and_skips_embeddings() {
        let catalog = LocalCatalog {
            servers: vec![LocalServer {
                base_url: "http://127.0.0.1:1234".into(),
                kind: LocalKind::LmStudio,
                models: vec![
                    LocalModel {
                        id: "z-llm".into(),
                        loaded: Some(true),
                        kind: "llm".into(),
                    },
                    LocalModel {
                        id: "a-embed".into(),
                        loaded: Some(true),
                        kind: "embeddings".into(),
                    },
                    LocalModel {
                        id: "b-llm".into(),
                        loaded: Some(true),
                        kind: "llm".into(),
                    },
                    LocalModel {
                        id: "c-idle".into(),
                        loaded: Some(false),
                        kind: "llm".into(),
                    },
                ],
            }],
        };
        let (_, chosen) = catalog.pick_loaded().unwrap();
        assert_eq!(chosen.id, "b-llm", "lowest loaded writing model wins");
        assert_eq!(catalog.loaded_count(), 3);
    }

    #[test]
    fn no_loaded_model_picks_nothing_rather_than_guessing() {
        let catalog = LocalCatalog {
            servers: vec![LocalServer {
                base_url: "http://127.0.0.1:1234".into(),
                kind: LocalKind::LmStudio,
                models: vec![LocalModel {
                    id: "x".into(),
                    loaded: Some(false),
                    kind: "llm".into(),
                }],
            }],
        };
        assert!(catalog.pick_loaded().is_none());
    }
}

#[cfg(test)]
mod live {
    use super::*;

    #[tokio::test]
    #[ignore = "probes the servers actually running on this machine"]
    async fn discovers_what_is_running_here() {
        let catalog = discover().await;
        for server in &catalog.servers {
            println!(
                "{} ({}): {} models, {} loaded",
                server.kind.label(),
                server.base_url,
                server.models.len(),
                server
                    .models
                    .iter()
                    .filter(|m| m.loaded == Some(true))
                    .count()
            );
        }
        println!(
            "choice: {:?}",
            catalog
                .pick_loaded()
                .map(|(s, m)| (s.base_url.clone(), m.id.clone()))
        );
    }
}
