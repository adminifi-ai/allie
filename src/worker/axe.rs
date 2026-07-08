use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AxeViolation {
    pub(crate) id: String,
    pub(crate) impact: Option<String>,
    pub(crate) help: Option<String>,
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    #[serde(default)]
    pub(crate) nodes: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AxeEvaluation {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    #[serde(default)]
    pub(crate) nodes: usize,
    pub(crate) viewport: AxeViewport,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AxeViewport {
    Desktop,
    Mobile,
}
