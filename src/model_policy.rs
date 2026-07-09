use crate::model_credentials::{MODEL_PROVIDER_PRESETS, ModelProviderPreset};
use crate::worker::RunFailure;
use crate::{AllieError, FlowManifest, ModelPolicy, Result};

impl FlowManifest {
    pub(crate) fn enforce_model_provider_allowlist(&self) -> Result<()> {
        if let Some(failure) = self.model.provider_allowlist_incomplete_failure() {
            return Err(AllieError::InvalidManifest(failure.message));
        }
        if let Some(failure) = self.model.provider_allowlist_failure() {
            return Err(AllieError::InvalidManifest(failure.message));
        }
        Ok(())
    }
}

pub(crate) struct ResolvedModelRoute {
    pub(crate) provider: String,
    pub(crate) model: String,
    pub(crate) api_key_env: String,
    pub(crate) base_url: String,
}

impl ModelPolicy {
    pub(crate) fn resolved_route(&self) -> ResolvedModelRoute {
        let provider = normalized_model_setting(&self.provider)
            .unwrap_or_else(|| default_model_provider_preset().provider.to_string());
        let preset = model_provider_preset(&provider).unwrap_or_else(default_model_provider_preset);
        ResolvedModelRoute {
            provider,
            model: normalized_model_setting(&self.model)
                .unwrap_or_else(|| preset.model.to_string()),
            api_key_env: normalized_model_setting(&self.api_key_env)
                .unwrap_or_else(|| preset.api_key_env.to_string()),
            base_url: normalized_model_setting(&self.base_url)
                .unwrap_or_else(|| preset.base_url.to_string()),
        }
    }

    pub(crate) fn provider_allowlist_incomplete_failure(&self) -> Option<RunFailure> {
        if self.enabled && self.effective_provider_allowlist().is_empty() {
            return Some(RunFailure::new(
                "model-policy-incomplete",
                "model-policy",
                "model calls are enabled but provider_allowlist is empty".to_string(),
            ));
        }
        None
    }

    /// Allowlist entries are canonical provider preset IDs. A model route is
    /// allowed only when the resolved provider is listed and the resolved
    /// base_url matches that provider preset; provider-name allowlisting alone
    /// must not bless a hostile endpoint override.
    pub(crate) fn provider_allowlist_failure(&self) -> Option<RunFailure> {
        let allowlist = self.effective_provider_allowlist();
        if !self.enabled || allowlist.is_empty() {
            return None;
        }

        let route = self.resolved_route();
        let preset = match model_provider_preset(&route.provider) {
            Some(preset) => preset,
            None => {
                return Some(RunFailure::new(
                    "model-policy-not-allowed",
                    "model-policy",
                    format!(
                        "model provider {} is not a known provider preset; provider_allowlist entries must name one of: {}",
                        route.provider,
                        model_provider_preset_names()
                    ),
                ));
            }
        };

        if !allowlist.contains(&route.provider.as_str()) {
            return Some(RunFailure::new(
                "model-policy-not-allowed",
                "model-policy",
                format!(
                    "model provider {} is not allowed by provider_allowlist [{}]; add {} or choose an allowlisted provider preset",
                    route.provider,
                    allowlist.join(", "),
                    route.provider
                ),
            ));
        }

        if normalize_base_url(&route.base_url) != normalize_base_url(preset.base_url) {
            return Some(RunFailure::new(
                "model-policy-not-allowed",
                "model-policy",
                format!(
                    "model provider {} is allowlisted only for preset base_url {}; resolved base_url {} is not allowed",
                    route.provider, preset.base_url, route.base_url
                ),
            ));
        }

        None
    }

    fn effective_provider_allowlist(&self) -> Vec<&str> {
        self.provider_allowlist
            .iter()
            .map(|entry| entry.trim())
            .filter(|entry| !entry.is_empty())
            .collect()
    }
}

fn default_model_provider_preset() -> &'static ModelProviderPreset {
    &MODEL_PROVIDER_PRESETS[0]
}

fn model_provider_preset(provider: &str) -> Option<&'static ModelProviderPreset> {
    let provider = provider.trim();
    MODEL_PROVIDER_PRESETS
        .iter()
        .find(|preset| preset.provider == provider)
}

fn model_provider_preset_names() -> String {
    MODEL_PROVIDER_PRESETS
        .iter()
        .map(|preset| preset.provider)
        .collect::<Vec<_>>()
        .join(", ")
}

fn normalized_model_setting(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn normalize_base_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests;
