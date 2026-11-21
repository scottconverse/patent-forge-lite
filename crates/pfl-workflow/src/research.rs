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
