//! Research sources. Live prior-art retrieval is a provider integration; this
//! module defines the seam it must implement and ships an explicit fixture
//! source so the pipeline can be exercised without any network access.
use crate::*;
use async_trait::async_trait;

/// A source of prior-art references for one invention. Implementations must not
/// be consulted unless the run authorized research, and their output is
/// snapshotted into the run input so a resume never silently re-queries.
#[async_trait]
pub trait ResearchSource: Send + Sync {
    async fn prior_art(
        &self,
        intake: &Intake,
        cancellation: CancellationToken,
    ) -> Result<Vec<PriorArtItem>>;
}

/// Explicit, offline fixture source. Never contacts a network and never invents
/// references that were not supplied by the caller.
#[derive(Debug, Clone, Default)]
pub struct FixtureResearchSource {
    items: Vec<PriorArtItem>,
}

impl FixtureResearchSource {
    pub fn new(items: Vec<PriorArtItem>) -> Self {
        Self { items }
    }
}

#[async_trait]
impl ResearchSource for FixtureResearchSource {
    async fn prior_art(
        &self,
        _intake: &Intake,
        cancellation: CancellationToken,
    ) -> Result<Vec<PriorArtItem>> {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        Ok(self.items.clone())
    }
}

/// A source that is configured but currently refuses to run. Used when research
/// is not authorized, so nothing silently degrades to an empty result.
pub struct DisabledResearchSource;

#[async_trait]
impl ResearchSource for DisabledResearchSource {
    async fn prior_art(
        &self,
        _intake: &Intake,
        _cancellation: CancellationToken,
    ) -> Result<Vec<PriorArtItem>> {
        Err(Error::Invalid(
            "research is not authorized for this run; no prior-art sources were queried".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fixture_source_returns_only_supplied_references() {
        let source = FixtureResearchSource::new(vec![PriorArtItem {
            reference: "US 1".into(),
            relevance: "fixture".into(),
            ..Default::default()
        }]);
        let items = source
            .prior_art(&Intake::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].reference, "US 1");
    }

    #[tokio::test]
    async fn disabled_source_refuses_instead_of_returning_nothing() {
        let error = DisabledResearchSource
            .prior_art(&Intake::default(), CancellationToken::new())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("not authorized"));
    }
}

/// An opt-in HTTP research source.
///
/// It speaks one documented contract so any provider adapter (or a local proxy)
/// can sit behind it:
///
///   GET {endpoint}?q={url-encoded technical field}&limit={n}
///   Authorization: Bearer {key}          (only when a key is configured)
///
/// expecting a JSON array of records:
///   [{ "reference": "...", "title": "...", "assignee": "...", "filing_date": "...",
///      "grant_date": "...", "cpc": "...", "url": "...", "relevance": "..." }]
///
/// Failure modes are distinct on purpose: a transport/HTTP error is an error, an
/// empty array is a successful search that found nothing, and a malformed body is
/// an error. "Search failed" is never reported as "no prior art exists".
pub struct HttpResearchSource {
    endpoint: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl HttpResearchSource {
    pub fn new(endpoint: impl Into<String>, api_key: Option<String>) -> Result<Self> {
        let endpoint = endpoint.into();
        let url = reqwest::Url::parse(&endpoint)
            .map_err(|_| Error::Invalid("invalid research endpoint URL".into()))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(Error::Invalid(
                "research endpoint must be an HTTP(S) URL with a host".into(),
            ));
        }
        // The generic source exists for loopback verification. Anywhere else it would send the
        // API key to a host the operator did not name as the real provider; live search is
        // PatentsView, not this.
        if !crate::providers::is_loopback_url(&endpoint) {
            return Err(Error::Invalid(
                "the generic research endpoint is for loopback verification only; live search uses PatentsView".into(),
            ));
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|_| Error::Provider("cannot initialize the research HTTP client".into()))?;
        Ok(Self {
            endpoint,
            api_key,
            client,
        })
    }
}

#[derive(serde::Deserialize)]
struct ResearchRecord {
    #[serde(default)]
    reference: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    assignee: String,
    #[serde(default)]
    filing_date: String,
    #[serde(default)]
    grant_date: String,
    #[serde(default)]
    cpc: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    relevance: String,
}

#[async_trait]
impl ResearchSource for HttpResearchSource {
    async fn prior_art(
        &self,
        intake: &Intake,
        cancellation: CancellationToken,
    ) -> Result<Vec<PriorArtItem>> {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let query = if intake.technical_field.trim().is_empty() {
            intake.title.clone()
        } else {
            intake.technical_field.clone()
        };
        let mut request = self
            .client
            .get(&self.endpoint)
            .query(&[("q", query.as_str()), ("limit", "10")]);
        if let Some(key) = &self.api_key {
            request = request.header("authorization", format!("Bearer {key}"));
        }
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(Error::Cancelled),
            result = request.send() => result.map_err(|_| Error::Provider(
                "research request failed; no prior-art set was recorded".into(),
            ))?,
        };
        if !response.status().is_success() {
            return Err(Error::Provider(format!(
                "research provider returned HTTP {}; no prior-art set was recorded",
                response.status().as_u16()
            )));
        }
        let body = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(Error::Cancelled),
            result = response.text() => result.map_err(|_| Error::Provider(
                "research response could not be read".into(),
            ))?,
        };
        let records: Vec<ResearchRecord> = serde_json::from_str(&body).map_err(|_| {
            Error::Provider(
                "research provider returned a malformed body; nothing was recorded".into(),
            )
        })?;
        Ok(records
            .into_iter()
            .filter(|record| !record.reference.trim().is_empty() || !record.title.trim().is_empty())
            .map(|record| PriorArtItem {
                reference: record.reference,
                title: record.title,
                assignee: record.assignee,
                filing_date: record.filing_date,
                grant_date: record.grant_date,
                cpc: record.cpc,
                url: record.url,
                relevance: record.relevance,
                source: "HTTP".into(),
                // Only PatentsView records are verified; an arbitrary HTTP source is not.
                verified: false,
                ..Default::default()
            })
            .collect())
    }
}

/// USPTO PatentsView adapter.
///
/// Uses the official search endpoint with the documented `X-Api-Key` header and
/// requests only the fields the disclosure record needs. Response parsing is
/// defensive: a record without an id or title is dropped rather than invented, and
/// a non-200 response is an error, never an empty result.
pub struct PatentsViewSource {
    endpoint: String,
    api_key: String,
    client: reqwest::Client,
}

impl PatentsViewSource {
    /// Production endpoint. The key is held by the caller for the session only.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::with_endpoint(api_key, "https://search.patentsview.org/api/v1/patent/")
    }

    /// Endpoint override for deterministic local fixtures.
    pub fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Result<Self> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(Error::Invalid("PatentsView requires an API key".into()));
        }
        let endpoint = endpoint.into();
        // The key travels in a custom header, which reqwest does not strip on a cross-host
        // redirect, so a redirect would hand the key to whoever the host points at next.
        // No redirects and no proxy: one request, to the configured host only.
        let url = reqwest::Url::parse(&endpoint)
            .map_err(|_| Error::Invalid("invalid PatentsView endpoint URL".into()))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || url.username() != ""
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::Invalid(
                "PatentsView endpoint must be HTTP(S), with a host and no credentials, query or fragment"
                    .into(),
            ));
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|_| Error::Provider("cannot initialize the PatentsView client".into()))?;
        // The endpoint is used verbatim: the API path is part of it.
        Ok(Self {
            endpoint,
            api_key,
            client,
        })
    }
}

#[async_trait]
impl ResearchSource for PatentsViewSource {
    async fn prior_art(
        &self,
        intake: &Intake,
        cancellation: CancellationToken,
    ) -> Result<Vec<PriorArtItem>> {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let query = if intake.technical_field.trim().is_empty() {
            intake.title.clone()
        } else {
            intake.technical_field.clone()
        };
        let body = serde_json::json!({
            "q": { "_text_any": { "patent_title": query, "patent_abstract": query } },
            "f": ["patent_id", "patent_title", "patent_date", "assignee_organization"],
            "o": { "per_page": 10 }
        });
        let request = self
            .client
            .post(&self.endpoint)
            .header("X-Api-Key", &self.api_key)
            .json(&body);
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(Error::Cancelled),
            result = request.send() => result.map_err(|_| Error::Provider(
                "PatentsView request failed; no prior-art set was recorded".into(),
            ))?,
        };
        if !response.status().is_success() {
            return Err(Error::Provider(format!(
                "PatentsView returned HTTP {}; no prior-art set was recorded",
                response.status().as_u16()
            )));
        }
        // Reading the body is part of the request: a cancel must not wait for a slow read to
        // finish before it takes effect.
        let payload: serde_json::Value = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(Error::Cancelled),
            body = response.json() => body.map_err(|_| {
                Error::Provider("PatentsView returned a malformed body; nothing was recorded".into())
            })?,
        };
        // "Search failed" must never be recorded as "no prior art exists": a body without a
        // `patents` array, or one that flags an error, is a failed search.
        if payload["error"].as_bool() == Some(true) {
            return Err(Error::Provider(
                "PatentsView reported an error; no prior-art set was recorded".into(),
            ));
        }
        let records = payload["patents"].as_array().cloned().ok_or_else(|| {
            Error::Provider("PatentsView returned no patents array; nothing was recorded".into())
        })?;
        Ok(records
            .into_iter()
            .filter_map(|record| {
                let number = record["patent_id"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let title = record["patent_title"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if number.is_empty() && title.is_empty() {
                    return None;
                }
                Some(PriorArtItem {
                    reference: if number.is_empty() {
                        title.clone()
                    } else {
                        number.clone()
                    },
                    publication_number: number,
                    title,
                    grant_date: record["patent_date"]
                        .as_str()
                        .unwrap_or("")
                        .trim()
                        .to_string(),
                    assignee: record["assignee_organization"]
                        .as_str()
                        .unwrap_or("")
                        .trim()
                        .to_string(),
                    source: "USPTO PatentsView".into(),
                    relevance: String::new(),
                    verified: true,
                    ..Default::default()
                })
            })
            .collect())
    }
}
