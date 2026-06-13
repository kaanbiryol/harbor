use std::collections::HashMap;

use harbor_domain::{MergeState, PullRequest};
use harbor_github::{GitHubRateLimitStatus, PullRequestEnrichment};

pub(super) fn merge_light_pull_request_rows(previous: &[PullRequest], current: &mut [PullRequest]) {
    let previous_by_number = previous
        .iter()
        .map(|pull_request| (pull_request.number, pull_request))
        .collect::<HashMap<_, _>>();

    for pull_request in current {
        let Some(previous_pull_request) = previous_by_number.get(&pull_request.number) else {
            continue;
        };

        if previous_pull_request.head_sha != pull_request.head_sha {
            continue;
        }

        if pull_request.node_id.is_empty() {
            pull_request.node_id = previous_pull_request.node_id.clone();
        }
        pull_request.review_decision = previous_pull_request.review_decision;
        pull_request.checks_summary = previous_pull_request.checks_summary;
        pull_request.unresolved_threads = previous_pull_request.unresolved_threads;
        if pull_request.merge_state == Some(MergeState::Unknown)
            || pull_request.merge_state.is_none()
        {
            pull_request.merge_state = previous_pull_request.merge_state;
        }
    }
}

pub(super) fn pull_request_enrichment_node_ids(
    current: &[PullRequest],
    force_enrichment: bool,
) -> Vec<String> {
    if !force_enrichment {
        return Vec::new();
    }

    current
        .iter()
        .filter(|pull_request| !pull_request.node_id.is_empty())
        .map(|pull_request| pull_request.node_id.clone())
        .collect()
}

pub(super) fn apply_pull_request_enrichments(
    pull_requests: &mut [PullRequest],
    enrichments: Vec<PullRequestEnrichment>,
) {
    let mut enrichments_by_node_id = enrichments
        .into_iter()
        .map(|enrichment| (enrichment.node_id.clone(), enrichment))
        .collect::<HashMap<_, _>>();

    for pull_request in pull_requests {
        let Some(enrichment) = enrichments_by_node_id.remove(&pull_request.node_id) else {
            continue;
        };

        pull_request.review_decision = enrichment.review_decision;
        pull_request.merge_state = enrichment.merge_state;
    }
}

pub(super) fn graphql_rate_limit_too_low_for_enrichment(
    rate_limits: &[GitHubRateLimitStatus],
) -> bool {
    rate_limits.iter().any(|rate_limit| {
        rate_limit.resource.as_deref() == Some("graphql")
            && match (rate_limit.remaining, rate_limit.limit) {
                (Some(remaining), Some(limit)) if limit > 0 => {
                    remaining <= 500 || remaining.saturating_mul(10) <= limit
                }
                (Some(remaining), None) => remaining <= 500,
                _ => false,
            }
    })
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeZone, Utc};
    use harbor_domain::{
        ChecksSummary, MergeState, PullRequest, PullRequestState, RepoId, ReviewDecision,
    };
    use harbor_github::PullRequestEnrichment;

    use super::{apply_pull_request_enrichments, merge_light_pull_request_rows};

    #[test]
    fn light_pull_request_merge_preserves_order_and_matching_row_fields() {
        let mut previous = pull_request(7);
        previous.node_id = "node-7".to_string();
        previous.review_decision = Some(ReviewDecision::Approved);
        previous.unresolved_threads = 3;
        previous.checks_summary = ChecksSummary {
            total: 2,
            passed: 1,
            failed: 1,
            pending: 0,
            skipped: 0,
        };
        let mut other_previous = pull_request(8);
        other_previous.node_id = "node-8".to_string();
        let mut current = vec![pull_request(8), pull_request(7)];
        current[0].node_id.clear();
        current[1].node_id.clear();

        merge_light_pull_request_rows(&[previous, other_previous], &mut current);

        assert_eq!(current[0].number, 8);
        assert_eq!(current[0].node_id, "node-8");
        assert_eq!(current[1].number, 7);
        assert_eq!(current[1].node_id, "node-7");
        assert_eq!(current[1].review_decision, Some(ReviewDecision::Approved));
        assert_eq!(current[1].unresolved_threads, 3);
        assert_eq!(current[1].checks_summary.failed, 1);
    }

    #[test]
    fn enrichment_application_preserves_order_and_updates_matching_rows() {
        let mut pull_requests = vec![pull_request(7), pull_request(8)];
        pull_requests[0].node_id = "node-7".to_string();
        pull_requests[1].node_id = "node-8".to_string();
        let enrichments = vec![PullRequestEnrichment {
            node_id: "node-8".to_string(),
            review_decision: Some(ReviewDecision::ChangesRequested),
            merge_state: Some(MergeState::Blocked),
            checks_summary: ChecksSummary::default(),
        }];

        apply_pull_request_enrichments(&mut pull_requests, enrichments);

        assert_eq!(pull_requests[0].number, 7);
        assert_eq!(pull_requests[0].review_decision, None);
        assert_eq!(pull_requests[1].number, 8);
        assert_eq!(
            pull_requests[1].review_decision,
            Some(ReviewDecision::ChangesRequested)
        );
        assert_eq!(pull_requests[1].merge_state, Some(MergeState::Blocked));
    }

    fn pull_request(number: u64) -> PullRequest {
        PullRequest {
            repo: RepoId::new("acme", "app"),
            node_id: format!("pr-{number}"),
            number,
            title: "Add feature".to_string(),
            body: None,
            author: "octocat".to_string(),
            url: format!("https://github.com/acme/app/pull/{number}"),
            state: PullRequestState::Open,
            is_draft: false,
            head_ref: "feature".to_string(),
            base_ref: "main".to_string(),
            head_sha: "abc123".to_string(),
            review_decision: None,
            merge_state: Some(MergeState::Clean),
            labels: Vec::new(),
            checks_summary: ChecksSummary {
                total: 1,
                passed: 0,
                failed: 0,
                pending: 1,
                skipped: 0,
            },
            unresolved_threads: 0,
            updated_at: Some(time(1)),
        }
    }

    fn time(minute: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 1, 10, minute as u32, 0)
            .single()
            .expect("valid test time")
    }
}
