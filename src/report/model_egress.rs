use crate::escape_html;
use crate::model::ModelEgressEvent;

pub(super) fn render(events: &[ModelEgressEvent]) -> String {
    if events.is_empty() {
        return String::new();
    }
    let successes = events
        .iter()
        .filter(|event| event.outcome == "success")
        .count();
    let prompt_tokens = events
        .iter()
        .filter_map(|event| event.usage.as_ref()?.prompt_tokens)
        .sum::<u64>();
    let completion_tokens = events
        .iter()
        .filter_map(|event| event.usage.as_ref()?.completion_tokens)
        .sum::<u64>();
    let cost_summary = reported_cost(events);
    let rows = events
        .iter()
        .map(|event| {
            let actual_route = match (&event.routed_provider, &event.routed_model) {
                (Some(provider), Some(model)) => {
                    format!("{} / {}", escape_html(provider), escape_html(model))
                }
                _ => "unavailable".to_string(),
            };
            let usage = event
                .usage
                .as_ref()
                .map(|usage| {
                    format!(
                        "{} prompt · {} completion · {} total · {} cost",
                        usage
                            .prompt_tokens
                            .map_or_else(|| "unavailable".to_string(), |value| value.to_string()),
                        usage.completion_tokens.map_or_else(
                            || "unavailable".to_string(),
                            |value| value.to_string()
                        ),
                        usage
                            .total_tokens
                            .map_or_else(|| "unavailable".to_string(), |value| value.to_string()),
                        usage
                            .cost
                            .map_or_else(|| "unavailable".to_string(), |value| format!("{value:.6}"))
                    )
                })
                .unwrap_or_else(|| "unavailable".to_string());
            format!(
                "<tr><td>{attempt}</td><td>{outcome}</td><td>{status}</td><td>{requested}</td><td>{actual}</td><td>{generation}</td><td><code>{prompt_version}</code><br><code>{prompt_hash}</code><br>{media_count} media hash(es)</td><td>{usage}</td></tr>",
                attempt = event.attempt,
                outcome = escape_html(&event.outcome),
                status = event
                    .http_status
                    .map_or_else(|| "—".to_string(), |value| value.to_string()),
                requested = escape_html(&format!(
                    "{} / {}",
                    event.requested_provider, event.requested_model
                )),
                actual = actual_route,
                generation = event
                    .generation_id
                    .as_deref()
                    .map_or_else(|| "unavailable".to_string(), escape_html),
                prompt_version = escape_html(&event.prompt_version),
                prompt_hash = escape_html(&event.prompt_sha256),
                media_count = event.media_sha256.len(),
                usage = usage,
            )
        })
        .collect::<String>();
    format!(
        "<section><h2 class=\"sh\">Model egress receipt</h2><div class=\"hard-panel\"><p><b>{attempts} HTTP attempt(s)</b> · {successes} successful · {failures} failed · {prompt_tokens} prompt tokens · {completion_tokens} completion tokens · {cost_summary}.</p><p class=\"sub\">ZDR and no-fallback policy is recorded per attempt. Prompt and media bodies, authorization values, and credentials are not included.</p><details class=\"matrix-wrap\"><summary>Per-attempt model egress evidence</summary><table class=\"matrix ledger\"><thead><tr><th>Attempt</th><th>Outcome</th><th>HTTP</th><th>Requested route</th><th>Actual route</th><th>Generation</th><th>Payload fingerprints</th><th>Usage</th></tr></thead><tbody>{rows}</tbody></table></details></div></section>",
        attempts = events.len(),
        successes = successes,
        failures = events.len() - successes,
        prompt_tokens = prompt_tokens,
        completion_tokens = completion_tokens,
        cost_summary = cost_summary,
        rows = rows,
    )
}

pub(super) fn summary(events: &[ModelEgressEvent]) -> String {
    if events.is_empty() {
        return "Model egress: no HTTP attempts.".to_string();
    }
    let successes = events
        .iter()
        .filter(|event| event.outcome == "success")
        .count();
    format!(
        "Model egress: {} HTTP attempt(s), {} successful, {} failed, {}. Per-attempt receipts are in the source packet.",
        events.len(),
        successes,
        events.len() - successes,
        reported_cost(events)
    )
}

fn reported_cost(events: &[ModelEgressEvent]) -> String {
    let costs = events
        .iter()
        .filter_map(|event| event.usage.as_ref()?.cost)
        .collect::<Vec<_>>();
    if costs.is_empty() {
        "reported cost unavailable".to_string()
    } else {
        format!(
            "{:.6} reported cost across {} attempt(s)",
            costs.iter().sum::<f64>(),
            costs.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModelEgressUsage;

    #[test]
    fn receipt_is_concise_disclosable_and_payload_free() {
        let event = ModelEgressEvent {
            schema: "allie.model-egress-event.v1".to_string(),
            attempt: 1,
            started_at: "2026-07-21T12:00:00Z".to_string(),
            requested_provider: "openrouter".to_string(),
            requested_model: "requested/model".to_string(),
            prompt_version: "allie.agentic.wcag-review.v1".to_string(),
            prompt_sha256: "a".repeat(64),
            media_sha256: vec!["b".repeat(64)],
            zdr_required: true,
            allow_fallbacks: false,
            outcome: "success".to_string(),
            http_status: Some(200),
            error_class: None,
            response_id: Some("response-1".to_string()),
            generation_id: Some("generation-1".to_string()),
            routed_provider: Some("Fake Provider".to_string()),
            routed_model: Some("actual/model".to_string()),
            usage: Some(ModelEgressUsage {
                prompt_tokens: Some(3),
                completion_tokens: Some(2),
                total_tokens: Some(5),
                cost: Some(0.001),
            }),
        };
        let html = render(std::slice::from_ref(&event));

        assert!(html.contains("1 HTTP attempt(s)"));
        assert!(html.contains("Per-attempt model egress evidence"));
        assert!(html.contains("requested/model"));
        assert!(html.contains("actual/model"));
        assert!(html.contains("generation-1"));
        assert!(html.contains(&"a".repeat(64)));
        assert!(!html.contains("Authorization"));
        assert!(!html.contains("api_key"));
        assert!(!html.contains("messages"));

        let mut unpriced = event;
        unpriced.usage.as_mut().unwrap().cost = None;
        let summary = summary(&[unpriced]);
        assert!(summary.contains("reported cost unavailable"));
        assert!(!summary.contains("0.000000 reported cost"));
    }
}
