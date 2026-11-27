//! Cost estimation from recorded usage.
//!
//! A total is computed only from recorded usage and a rate this build knows about. The rates
//! in `pricing_for` are a documented table of published list prices for the models it names;
//! for any other model there is no rate, so the estimate is `None` and the window says the
//! cost is unknown rather than inventing one. When any stage's usage is unknown the estimate is
//! `None` too — "unknown", never `0.0`, which would read as free.
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
/// Rates recorded when this build was made, used only to disclose what a paid run will
/// be billed at. They are a basis for the confirmation line, not a quote: the total is
/// unknown until the provider reports usage. An unknown model yields `None`, so the
/// window says plainly that no rates are configured rather than inventing them.
/// Exact model ids and the per-million rates recorded for them on 2026-11-27.
///
/// A family substring used to decide this, so a model released after that date inherited its
/// predecessor's price and the confirmation line stated a figure nobody had checked. An id that is
/// not in this table yields `None`, which the window renders as "no rates are configured".
const RATES: &[(&str, f64, f64)] = &[
    ("claude-opus-4-1", 15.0, 75.0),
    ("claude-sonnet-4-5", 3.0, 15.0),
    ("claude-sonnet-4", 3.0, 15.0),
    ("claude-haiku-4-5", 0.8, 4.0),
];

pub fn pricing_for(model: &str) -> Option<Pricing> {
    let model = model.trim().to_ascii_lowercase();
    RATES
        .iter()
        .find(|(id, _, _)| *id == model)
        .map(|(_, input, output)| Pricing {
            input_per_million: *input,
            output_per_million: *output,
        })
}

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
    fn the_confirmation_line_names_the_rates_that_will_apply() {
        let rates = pricing_for("claude-sonnet-4").expect("a known model has recorded rates");
        assert_eq!(rates.input_per_million, 3.0);
        assert_eq!(rates.output_per_million, 15.0);
        let text = confirmation_text("claude-sonnet-4", Some(&rates));
        assert!(
            text.contains("unknown until the run reports usage"),
            "{text}"
        );
        assert!(text.contains("3.00 input"), "{text}");
        assert!(text.contains("15.00 output"), "{text}");
    }

    #[test]
    fn an_unknown_model_says_no_rates_rather_than_inventing_them() {
        assert_eq!(pricing_for("mystery-model"), None);
        let text = confirmation_text("mystery-model", pricing_for("mystery-model").as_ref());
        assert!(text.contains("No rates are configured"), "{text}");
        assert!(!text.contains("per 1M tokens"), "{text}");
    }
    #[test]
    fn zero_tokens_with_no_unknowns_is_a_real_zero() {
        let usage = UsageSummary::default();
        let estimate = estimate(&usage, &pricing()).expect("no unknowns means a real total");
        assert_eq!(estimate.amount, 0.0);
    }

    #[test]
    fn a_model_that_is_not_listed_is_refused_rather_than_given_a_family_rate() {
        // A later model in the same family must not inherit the older price.
        for unknown in [
            "claude-sonnet-4-6",
            "claude-opus-4-2",
            "sonnet",
            "opus",
            "haiku",
        ] {
            assert_eq!(
                pricing_for(unknown),
                None,
                "{unknown} must not be priced by family"
            );
        }
        // The exact ids that were recorded still price.
        assert_eq!(
            pricing_for("claude-sonnet-4").map(|p| p.input_per_million),
            Some(3.0)
        );
        assert_eq!(
            pricing_for("claude-opus-4-1").map(|p| p.output_per_million),
            Some(75.0)
        );
    }
}
