//! Authenticated-audit recipe types (ticket 023, epic 015 slice 1).
//!
//! An `AuthFlow` is an optional block on a `FlowManifest` describing how to
//! establish a session before a gated route is audited. The cardinal invariant:
//! **credential VALUES never appear here.** A step carries only the env-var
//! *name* (`value_env`); the Node worker reads the secret value from its own
//! inherited `process.env` at run time. This is what keeps secrets off disk —
//! they are never serialized into `worker-request.json`, the evidence packet, or
//! any artifact.
//!
//! Two ways to authenticate:
//!   * `steps` — a deterministic form-login recipe (fill / click / wait_for).
//!   * `storage_state_env` — an escape hatch for SSO/OAuth: the named env var
//!     points at a Playwright `storageState` JSON file captured out of band.
//!
//! The `authenticated_marker` is the no-silent-gaps mechanism: every gated state
//! must show it after navigation, else the worker records an `auth-lost`
//! `state_error` which flips the run to a blocking exit class. An HTTP-200 login
//! wall therefore blocks instead of being audited as if it were the app.

use serde::{Deserialize, Serialize};

/// Optional authentication recipe attached to a `FlowManifest`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct AuthFlow {
    /// Route the login recipe starts on (e.g. `/login.html`). Worker-only.
    #[serde(default)]
    pub start_path: Option<String>,
    /// Ordered login steps. Empty is only valid alongside `storage_state_env`.
    #[serde(default)]
    pub steps: Vec<AuthStep>,
    /// Selector / url marker that must be present on every gated state. Its
    /// absence after navigation means the session was lost and the run blocks.
    #[serde(default)]
    pub authenticated_marker: Option<AuthAssert>,
    /// SSO/OAuth hatch: env-var NAME of a path to a Playwright storageState file.
    #[serde(default)]
    pub storage_state_env: Option<String>,
}

/// A single login step. Modelled as an untagged enum of single-key maps
/// (`{ fill: {...} }`, `{ click: {...} }`, `{ wait_for: {...} }`) so the manifest
/// reads as a flat step list and the worker branches on `step.fill` / `step.click`
/// / `step.wait_for`. (serde_yaml 0.9 cannot deserialize the externally-tagged
/// `{fill: ...}` form without a YAML `!fill` tag, so untagged single-key wrappers
/// are the shape that keeps the intended manifest syntax.)
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub(crate) enum AuthStep {
    /// Fill `fill.selector` with the VALUE of env var `fill.value_env`. The step
    /// carries only the env NAME — never the credential value.
    Fill { fill: AuthFill },
    /// Click `click.selector`.
    Click { click: AuthClick },
    /// Wait for a success signal: a selector present or the URL containing a
    /// fragment.
    WaitFor { wait_for: AuthAssert },
}

/// Body of a `fill` step. `value_env` is the env-var NAME holding the value.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct AuthFill {
    pub selector: String,
    pub value_env: String,
}

/// Body of a `click` step.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct AuthClick {
    pub selector: String,
}

/// A success/marker assertion: a selector that must be present and/or a URL
/// fragment the final URL must contain. At least one must be set.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct AuthAssert {
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub url_contains: Option<String>,
}

impl AuthAssert {
    /// An assertion is meaningless unless it names a selector or a URL fragment.
    pub fn is_empty(&self) -> bool {
        self.selector.is_none() && self.url_contains.is_none()
    }
}

impl AuthFlow {
    /// Env-var names referenced by `fill` steps (the credentials this flow needs
    /// in the environment). Never includes any value.
    pub fn referenced_value_envs(&self) -> Vec<&str> {
        self.steps
            .iter()
            .filter_map(|step| match step {
                AuthStep::Fill { fill } => Some(fill.value_env.as_str()),
                _ => None,
            })
            .collect()
    }

    /// All `AuthAssert`s declared on this flow (marker + every `wait_for`).
    pub fn assertions(&self) -> Vec<&AuthAssert> {
        let mut out: Vec<&AuthAssert> = self
            .steps
            .iter()
            .filter_map(|step| match step {
                AuthStep::WaitFor { wait_for } => Some(wait_for),
                _ => None,
            })
            .collect();
        if let Some(marker) = &self.authenticated_marker {
            out.push(marker);
        }
        out
    }
}
