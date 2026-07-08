use crate::model_credentials::{MODEL_PROVIDER_PRESETS, ModelProviderPreset};
use crate::worker::RunFailure;
use crate::{AllieError, FlowManifest, ModelPolicy, Result};

impl FlowManifest {
    pub(crate) fn enforce_model_provider_allowlist(&self) -> Result<()> {
        if let Some(failure) = self.model.provider_allowlist_failure() {
            return Err(AllieError::InvalidManifest(failure.message));
        }
        Ok(())
    }
}

impl ModelPolicy {
    pub(crate) fn resolved_provider(&self) -> String {
        normalized_model_setting(&self.provider)
            .unwrap_or_else(|| default_model_provider_preset().provider.to_string())
    }

    pub(crate) fn resolved_base_url(&self) -> String {
        normalized_model_setting(&self.base_url).unwrap_or_else(|| {
            model_provider_preset(&self.resolved_provider())
                .unwrap_or_else(default_model_provider_preset)
                .base_url
                .to_string()
        })
    }

    /// Allowlist entries are canonical provider preset IDs. A model route is
    /// allowed only when the resolved provider is listed and the resolved
    /// base_url matches that provider preset; provider-name allowlisting alone
    /// must not bless a hostile endpoint override.
    pub(crate) fn provider_allowlist_failure(&self) -> Option<RunFailure> {
        if !self.enabled || self.provider_allowlist.is_empty() {
            return None;
        }

        let provider = self.resolved_provider();
        let base_url = self.resolved_base_url();
        let allowlist = self
            .provider_allowlist
            .iter()
            .map(|entry| entry.trim())
            .filter(|entry| !entry.is_empty())
            .collect::<Vec<_>>();
        let preset = match model_provider_preset(&provider) {
            Some(preset) => preset,
            None => {
                return Some(RunFailure::new(
                    "model-policy-not-allowed",
                    "model-policy",
                    format!(
                        "model provider {provider} is not a known provider preset; provider_allowlist entries must name one of: {}",
                        model_provider_preset_names()
                    ),
                ));
            }
        };

        if !allowlist.contains(&provider.as_str()) {
            return Some(RunFailure::new(
                "model-policy-not-allowed",
                "model-policy",
                format!(
                    "model provider {provider} is not allowed by provider_allowlist [{}]; add {provider} or choose an allowlisted provider preset",
                    allowlist.join(", ")
                ),
            ));
        }

        if normalize_base_url(&base_url) != normalize_base_url(preset.base_url) {
            return Some(RunFailure::new(
                "model-policy-not-allowed",
                "model-policy",
                format!(
                    "model provider {provider} is allowlisted only for preset base_url {}; resolved base_url {base_url} is not allowed",
                    preset.base_url
                ),
            ));
        }

        None
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
