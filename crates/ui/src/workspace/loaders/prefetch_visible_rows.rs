use std::{collections::HashMap, ops::Range};

use gpui::Context;
use harbor_domain::{MergeState, PullRequest};
use harbor_github::PullRequestEnrichment;

use crate::workspace::{
    AppView, PullRequestInboxCacheKey, PullRequestRowEnrichmentKey,
    async_updates::AppViewAsyncUpdateExt,
};

impl AppView {
    pub(crate) fn prefetch_visible_pull_request_row_enrichments(
        &mut self,
        range: Range<usize>,
        cx: &mut Context<Self>,
    ) {
        let Some(inbox_key) = self.current_pull_request_inbox_key() else {
            return;
        };
        let row_enrichment_requests =
            self.visible_pull_request_row_enrichment_requests(inbox_key.clone(), range);
        if row_enrichment_requests.is_empty() {
            return;
        }

        let node_ids = row_enrichment_requests
            .iter()
            .map(|key| key.node_id().to_string())
            .collect::<Vec<_>>();
        let github_api = self.github_api.clone();

        cx.spawn(async move |this, cx| {
            let result = github_api.enrich_pull_requests_by_node_ids(&node_ids).await;

            this.update_or_log(
                cx,
                "failed to update visible pull request row enrichment",
                move |view, cx| {
                    if view.current_pull_request_inbox_key().as_ref() != Some(&inbox_key) {
                        return;
                    }

                    match result {
                        Ok(enrichments) => {
                            if view.apply_visible_pull_request_row_enrichments(
                                &inbox_key,
                                &row_enrichment_requests,
                                enrichments,
                            ) {
                                view.cache_current_pull_request_inbox_snapshot();
                                cx.notify();
                            }
                        }
                        Err(error) => {
                            tracing::warn!(
                                repository = %inbox_key.repository().full_name(),
                                mode = inbox_key.mode().key(),
                                %error,
                                "failed to prefetch visible pull request row enrichment"
                            );
                        }
                    }
                },
            );
        })
        .detach();
    }

    fn visible_pull_request_row_enrichment_requests(
        &mut self,
        inbox_key: PullRequestInboxCacheKey,
        range: Range<usize>,
    ) -> Vec<PullRequestRowEnrichmentKey> {
        let mut keys = Vec::new();

        for index in range {
            let Some(pull_request) = self.pull_requests.get(index) else {
                continue;
            };
            if !should_prefetch_pull_request_row_enrichment(pull_request) {
                continue;
            }
            let Some(key) = PullRequestRowEnrichmentKey::new(inbox_key.clone(), pull_request)
            else {
                continue;
            };
            if !self
                .pull_request_inbox
                .mark_row_enrichment_attempted(key.clone())
            {
                continue;
            }

            keys.push(key);
        }

        keys
    }

    fn apply_visible_pull_request_row_enrichments(
        &mut self,
        inbox_key: &PullRequestInboxCacheKey,
        requested_keys: &[PullRequestRowEnrichmentKey],
        enrichments: Vec<PullRequestEnrichment>,
    ) -> bool {
        let mut enrichments_by_node_id = enrichments
            .into_iter()
            .map(|enrichment| (enrichment.node_id.clone(), enrichment))
            .collect::<HashMap<_, _>>();
        let mut changed = false;

        for pull_request in &mut self.pull_requests {
            let Some(row_key) = PullRequestRowEnrichmentKey::new(inbox_key.clone(), pull_request)
            else {
                continue;
            };
            if !requested_keys.contains(&row_key) {
                continue;
            }
            let Some(enrichment) = enrichments_by_node_id.remove(&pull_request.node_id) else {
                continue;
            };

            if pull_request.review_decision != enrichment.review_decision {
                pull_request.review_decision = enrichment.review_decision;
                changed = true;
            }
            if let Some(merge_state) = enrichment.merge_state
                && pull_request.merge_state != Some(merge_state)
            {
                pull_request.merge_state = Some(merge_state);
                changed = true;
            }
        }

        changed
    }
}

fn should_prefetch_pull_request_row_enrichment(pull_request: &PullRequest) -> bool {
    pull_request.review_decision.is_none()
        || pull_request.merge_state.is_none()
        || pull_request.merge_state == Some(MergeState::Unknown)
}
