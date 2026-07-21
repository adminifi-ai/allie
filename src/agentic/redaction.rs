use crate::{AllieError, ModelRedactionMode, Result};

const RECEIPT_SCHEMA: &str = "allie.model-redaction-receipt.v0";

#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Status {
    NotSent,
    NotApplied,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Receipt {
    schema: String,
    pub(super) profile: ModelRedactionMode,
    status: Status,
}

pub(super) fn accepted_redaction_receipt(
    response: &serde_json::Value,
    expected_profile: Option<ModelRedactionMode>,
) -> Result<Receipt> {
    let value = response.get("redaction_receipt").ok_or_else(|| {
        AllieError::Worker("agentic worker response omitted redaction_receipt".to_string())
    })?;
    let receipt: Receipt = serde_json::from_value(value.clone()).map_err(|_| {
        AllieError::Worker(
            "agentic worker returned an invalid or unsupported redaction_receipt".to_string(),
        )
    })?;
    if receipt.schema != RECEIPT_SCHEMA {
        return Err(AllieError::Worker(format!(
            "agentic worker returned redaction receipt schema {}; expected {RECEIPT_SCHEMA}",
            receipt.schema
        )));
    }
    if Some(receipt.profile) != expected_profile {
        return Err(AllieError::Worker(
            "agentic worker redaction receipt contradicts the accepted manifest mode".to_string(),
        ));
    }
    let calls = response["calls"].as_u64().ok_or_else(|| {
        AllieError::Worker("agentic worker response omitted a numeric calls count".to_string())
    })?;
    let expected_status = if calls == 0 {
        Status::NotSent
    } else {
        Status::NotApplied
    };
    if receipt.status != expected_status {
        return Err(AllieError::Worker(
            "agentic worker redaction receipt contradicts the model call count".to_string(),
        ));
    }
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_contradictory_and_fabricated_applied_claims() {
        let expected = Some(ModelRedactionMode::None);
        assert!(accepted_redaction_receipt(&serde_json::json!({ "calls": 0 }), expected).is_err());
        let contradictory = serde_json::json!({
            "calls": 1,
            "redaction_receipt": {
                "schema": RECEIPT_SCHEMA, "profile": "none", "status": "not_sent"
            }
        });
        assert!(accepted_redaction_receipt(&contradictory, expected).is_err());
        for status in ["applied", "redacted"] {
            let fabricated = serde_json::json!({
                "calls": 1,
                "redaction_receipt": {
                    "schema": RECEIPT_SCHEMA, "profile": "none", "status": status
                }
            });
            assert!(accepted_redaction_receipt(&fabricated, expected).is_err());
        }
    }
}
