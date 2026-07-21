//! Presets for the model gateway providers `allie init` and `allie doctor`
//! can detect credentials for automatically. Split out of `lib.rs` (where
//! `ModelPolicy` itself still lives) so this table and its tests don't grow
//! the crate root's line count.

use crate::ModelPolicy;

/// A provider preset the agentic model gateway (`workers/agentic/review.mjs`)
/// can call: it speaks a generic OpenAI-compatible `/chat/completions` API
/// keyed off whatever `model.api_key_env` names, so more presets can be added
/// here without gateway changes. Order is the priority `allie init` and
/// `allie doctor` probe them in.
pub(crate) struct ModelProviderPreset {
    pub(crate) provider: &'static str,
    pub(crate) api_key_env: &'static str,
    pub(crate) base_url: &'static str,
    pub(crate) model: &'static str,
}

pub(crate) const MODEL_PROVIDER_PRESETS: &[ModelProviderPreset] = &[
    ModelProviderPreset {
        provider: "openrouter",
        api_key_env: "OPENROUTER_API_KEY",
        base_url: "https://openrouter.ai/api/v1",
        model: "google/gemini-3.5-flash",
    },
    ModelProviderPreset {
        provider: "openai",
        api_key_env: "OPENAI_API_KEY",
        base_url: "https://api.openai.com/v1",
        model: "gpt-4o-mini",
    },
];

/// The first preset (in priority order) whose `api_key_env` resolves to a
/// non-empty value in the current process environment, if any.
pub(crate) fn resolve_model_credentials() -> Option<&'static ModelProviderPreset> {
    MODEL_PROVIDER_PRESETS
        .iter()
        .find(|preset| env_var_non_empty(preset.api_key_env))
}

/// The env var names `resolve_model_credentials` checks, for diagnostics when
/// none of them resolve.
pub(crate) fn model_provider_preset_env_names() -> String {
    MODEL_PROVIDER_PRESETS
        .iter()
        .map(|preset| preset.api_key_env)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn env_var_non_empty(name: &str) -> bool {
    std::env::var(name)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

/// The model policy `allie init` scaffolds: enabled + provider_allowlist
/// filled when a key resolves (never enabled with an empty allowlist — that
/// fails the manifest's fail-closed preflight gate), else default.
pub(crate) fn scaffold_model_policy() -> ModelPolicy {
    match resolve_model_credentials() {
        Some(preset) => ModelPolicy {
            enabled: true,
            provider_allowlist: vec![preset.provider.to_string()],
            zdr_required: preset.provider == "openrouter",
            redaction: Some(crate::ModelRedactionMode::None),
            provider: Some(preset.provider.to_string()),
            model: Some(preset.model.to_string()),
            api_key_env: Some(preset.api_key_env.to_string()),
            base_url: Some(preset.base_url.to_string()),
            ..ModelPolicy::default()
        },
        None => ModelPolicy::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FlowManifest;
    use crate::test_support::MODEL_ENV_GUARD;
    use std::path::Path;

    fn clear_model_credential_env() {
        unsafe {
            std::env::remove_var("OPENROUTER_API_KEY");
            std::env::remove_var("OPENAI_API_KEY");
        }
    }

    #[test]
    fn resolve_model_credentials_returns_none_when_no_key_is_set() {
        let _guard = MODEL_ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_model_credential_env();

        assert!(resolve_model_credentials().is_none());
    }

    #[test]
    fn resolve_model_credentials_prefers_openrouter_over_openai() {
        let _guard = MODEL_ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_model_credential_env();
        unsafe {
            std::env::set_var("OPENROUTER_API_KEY", "sk-or-test");
            std::env::set_var("OPENAI_API_KEY", "sk-openai-test");
        }

        let preset = resolve_model_credentials().expect("a preset should resolve");

        clear_model_credential_env();

        assert_eq!(preset.provider, "openrouter");
        assert_eq!(preset.api_key_env, "OPENROUTER_API_KEY");
    }

    #[test]
    fn resolve_model_credentials_falls_back_to_openai_when_openrouter_key_absent() {
        let _guard = MODEL_ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_model_credential_env();
        unsafe {
            std::env::set_var("OPENAI_API_KEY", "sk-openai-test");
        }

        let preset = resolve_model_credentials().expect("a preset should resolve");

        clear_model_credential_env();

        assert_eq!(preset.provider, "openai");
        assert_eq!(preset.api_key_env, "OPENAI_API_KEY");
    }

    #[test]
    fn resolve_model_credentials_treats_a_blank_value_as_unset() {
        let _guard = MODEL_ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_model_credential_env();
        unsafe {
            std::env::set_var("OPENROUTER_API_KEY", "   ");
        }

        let resolved = resolve_model_credentials();

        clear_model_credential_env();

        assert!(resolved.is_none());
    }

    #[test]
    fn model_policy_scaffold_enables_and_fills_provider_allowlist_when_key_resolves() {
        let _guard = MODEL_ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_model_credential_env();
        unsafe {
            std::env::set_var("OPENROUTER_API_KEY", "sk-or-test");
        }

        let policy = scaffold_model_policy();

        clear_model_credential_env();

        assert!(policy.enabled);
        assert_eq!(policy.provider_allowlist, vec!["openrouter".to_string()]);
        assert_eq!(policy.redaction, Some(crate::ModelRedactionMode::None));
        assert_eq!(policy.provider.as_deref(), Some("openrouter"));
        assert_eq!(policy.api_key_env.as_deref(), Some("OPENROUTER_API_KEY"));
        assert!(policy.model.is_some());
        assert!(policy.base_url.is_some());
    }

    #[test]
    fn model_policy_scaffold_stays_disabled_when_no_key_resolves() {
        let _guard = MODEL_ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_model_credential_env();

        let policy = scaffold_model_policy();

        assert!(!policy.enabled);
        assert!(policy.provider_allowlist.is_empty());
    }

    #[test]
    fn model_policy_scaffold_never_trips_the_fail_closed_allowlist_gate() {
        let _guard = MODEL_ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_model_credential_env();
        unsafe {
            std::env::set_var("OPENROUTER_API_KEY", "sk-or-test");
        }

        let mut manifest = FlowManifest::load(Path::new("examples/login-flow.yml")).unwrap();
        manifest.model = scaffold_model_policy();
        let failures = manifest.preflight_failures();

        clear_model_credential_env();

        assert!(
            !failures
                .iter()
                .any(|failure| failure.kind == "model-policy-incomplete"),
            "scaffolded model policy must not fail the enabled-without-allowlist gate: {failures:?}"
        );
    }
}
