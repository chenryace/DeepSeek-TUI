//! Provider model offerings (#3084).
//!
//! A [`ProviderModelOffering`] binds a provider to a canonical model, the
//! provider-owned wire id that serves it, and the endpoint key. This is the
//! seam that proves the #2608 invariant: the SAME canonical model can be served
//! by multiple providers under DIFFERENT wire ids (some aggregator-prefixed),
//! and a prefix never implies provider ownership.
//!
//! [`BUNDLED_OFFERINGS`] is intentionally tiny: a couple DeepSeek-native rows
//! plus a couple aggregator rows (Together / OpenRouter) whose wire ids carry
//! prefixes such as `deepseek-ai/DeepSeek-V4-Pro`. It exists to exercise the
//! seam, not to be the eventual catalog.

use super::ids::{ModelId, ProviderId, WireModelId};

/// One provider's way of serving a (possibly canonical) model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderModelOffering {
    /// Provider serving this offering.
    pub provider: ProviderId,
    /// Canonical model identity, if this offering maps to one.
    pub canonical_model: Option<ModelId>,
    /// Provider-owned wire id sent on the request (verbatim).
    pub wire_model_id: WireModelId,
    /// Endpoint key the offering is served on.
    pub endpoint_key: String,
    /// Whether this is the provider's default offering.
    pub default_for_provider: bool,
}

/// A static, lazily-materialized seam catalog.
///
/// Each row binds a provider id, an optional canonical model id, the wire id
/// it is served under, the endpoint key, and whether it is the provider
/// default. Aggregator rows demonstrate prefixed wire ids.
struct OfferingSeed {
    provider: &'static str,
    canonical_model: Option<&'static str>,
    wire_model_id: &'static str,
    endpoint_key: &'static str,
    default_for_provider: bool,
}

const OFFERING_SEEDS: &[OfferingSeed] = &[
    // DeepSeek-native: wire id equals the bare model name, no prefix.
    OfferingSeed {
        provider: "deepseek",
        canonical_model: Some("deepseek-v4-pro"),
        wire_model_id: "deepseek-v4-pro",
        endpoint_key: "chat",
        default_for_provider: true,
    },
    OfferingSeed {
        provider: "deepseek",
        canonical_model: Some("deepseek-v4-flash"),
        wire_model_id: "deepseek-v4-flash",
        endpoint_key: "chat",
        default_for_provider: false,
    },
    // Together aggregator: same canonical model, prefixed wire id.
    OfferingSeed {
        provider: "together",
        canonical_model: Some("deepseek-v4-pro"),
        wire_model_id: "deepseek-ai/DeepSeek-V4-Pro",
        endpoint_key: "chat",
        default_for_provider: true,
    },
    // OpenRouter aggregator: same canonical model, different prefixed wire id.
    OfferingSeed {
        provider: "openrouter",
        canonical_model: Some("deepseek-v4-pro"),
        wire_model_id: "deepseek/deepseek-v4-pro",
        endpoint_key: "chat",
        default_for_provider: true,
    },
];

/// Return the bundled offering seam as owned [`ProviderModelOffering`] rows.
///
/// Owned because the newtypes wrap `String`; the seed table stays `&'static`.
#[must_use]
pub fn bundled_offerings() -> Vec<ProviderModelOffering> {
    OFFERING_SEEDS
        .iter()
        .map(|seed| ProviderModelOffering {
            provider: ProviderId::from(seed.provider),
            canonical_model: seed.canonical_model.map(ModelId::from),
            wire_model_id: WireModelId::from(seed.wire_model_id),
            endpoint_key: seed.endpoint_key.to_string(),
            default_for_provider: seed.default_for_provider,
        })
        .collect()
}
