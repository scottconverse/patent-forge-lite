//! Cost estimation from recorded usage.
//!
//! Prices are **supplied by the operator**; this module never invents a price and
//! never claims a total it cannot justify. When any stage's usage is unknown the
//! estimate is `None` — "unknown", never `0.0`, which would read as free.
use crate::{UsageSummary, usage_summary};
use pfl_domain::RunSnapshot;
use serde::{Deserialize, Serialize};

/// Operator-supplied price for one model, in currency units per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pricing {
    pub input_per_million: f64,
    pub output_per_million: f64,
}

/// An estimate with the basis attached, so a confirmation screen can state how the
/// number was produced rather than showing a bare figure.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Estimate {
    pub amount: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl Estimate {
    pub fn basis(&self, pricing: &Pricing) -> String {
        format!(
            "{:.2} input + {:.2} output per 1M tokens; this run used {} input and {} output tokens",
            pricing.input_per_million,
            pricing.output_per_million,
            self.input_tokens,
            self.output_tokens
        )
    }
}

/// Estimates the cost of recorded usage at the given price.
///
/// Returns `None` if any stage's usage is unknown, because a partial total
/// understates the bill.
pub fn estimate(usage: &UsageSummary, pricing: &Pricing) -> Option<Estimate> {
    if usage.input_unknown > 0 || usage.output_unknown > 0 {
        return None;
    }
    let amount = (usage.input_tokens as f64 / 1_000_000.0) * pricing.input_per_million
        + (usage.output_tokens as f64 / 1_000_000.0) * pricing.output_per_million;
    Some(Estimate {
        amount,
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
    })
}

/// Convenience for a run, using the same usage summary the usage records report.
pub fn estimate_run(run: &RunSnapshot, pricing: &Pricing) -> Option<Estimate> {
    estimate(&usage_summary(run), pricing)
}

/// The text a confirmation screen shows before a paid run.
///
/// A run's usage is unknowable in advance, so the honest statement is *unknown*
/// plus the rates that will be applied — never a fabricated total.
pub fn confirmation_text(model: &str, pricing: Option<&Pricing>) -> String {
    match pricing {
        Some(rates) => format!(
            "{model}: cost unknown until the run reports usage. Rates applied: {:.2} input and {:.2} output per 1M tokens.",
            rates.input_per_million, rates.output_per_million
        ),
        None => format!(
            "{model}: cost unknown. No rates are configured for this model, so no total can be stated."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pricing() -> Pricing {
        Pricing {
            input_per_million: 3.0,
            output_per_million: 15.0,
        }
    }

    #[test]
    fn a_confirmation_states_unknown_and_the_rates_rather_than_a_total() {
        let text = confirmation_text("claude-test", Some(&pricing()));
        assert!(text.contains("cost unknown until the run reports usage"));
        assert!(text.contains("3.00 input"));
        assert!(text.contains("15.00 output"));
        assert!(
            !text.contains('$'),
            "no currency figure may be invented before usage exists"
        );
    }

    #[test]
    fn a_confirmation_without_rates_says_only_unknown() {
        let text = confirmation_text("claude-test", None);
        assert!(text.contains("cost unknown"));
        assert!(text.contains("No rates are configured"));
        assert!(!text.contains('$'));
    }

    #[test]
    fn known_usage_is_priced_from_the_supplied_rates() {
        let usage = UsageSummary {
            input_tokens: 1_000_000,
            output_tokens: 200_000,
            input_unknown: 0,
            output_unknown: 0,
        };
        let estimate = estimate(&usage, &pricing()).expect("fully known usage estimates");
        assert!((estimate.amount - 6.0).abs() < 1e-9, "3.0 + 3.0");
        assert!(estimate.basis(&pricing()).contains("3.00 input"));
    }

    #[test]
    fn unknown_usage_yields_no_estimate_rather_than_zero() {
        let usage = UsageSummary {
            input_tokens: 500,
            output_tokens: 500,
            input_unknown: 1,
            output_unknown: 0,
        };
        assert_eq!(estimate(&usage, &pricing()), None);
    }

    #[test]
    fn zero_tokens_with_no_unknowns_is_a_real_zero() {
        let usage = UsageSummary::default();
        let estimate = estimate(&usage, &pricing()).expect("no unknowns means a real total");
        assert_eq!(estimate.amount, 0.0);
    }
}
