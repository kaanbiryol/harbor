use std::time::Duration;

use crate::{
    GitHubApiFamily, GitHubRateLimitStatus, GitHubRequestAttribution,
    transport::response::GitHubRateLimitMetadata,
};

use super::GhCliRequestCoordinator;

const MAX_REQUEST_ATTRIBUTION_HISTORY: usize = 100;

impl GhCliRequestCoordinator {
    pub(in crate::transport) fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latest_rate_limit
            .clone()
    }

    pub(in crate::transport) fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latest_rate_limits
            .values()
            .cloned()
            .collect()
    }

    pub(in crate::transport) fn latest_request_attribution(
        &self,
    ) -> Option<GitHubRequestAttribution> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .latest_request_attribution
            .clone()
    }

    pub(in crate::transport) fn recent_request_attributions(
        &self,
    ) -> Vec<GitHubRequestAttribution> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .recent_request_attributions
            .iter()
            .cloned()
            .collect()
    }

    pub(in crate::transport) fn record_rate_limit_and_attribution(
        &self,
        family: GitHubApiFamily,
        operation_name: String,
        rate_limit: &GitHubRateLimitMetadata,
        duration: Duration,
        graphql_cost: Option<u64>,
    ) {
        let Some(rate_limit_status) = rate_limit.clone().into_status() else {
            return;
        };

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let resource = rate_limit_status
            .resource
            .clone()
            .unwrap_or_else(|| family.label().to_string());
        let previous = state.latest_rate_limits.get(&resource);
        let spent = rate_limit_spent(previous, &rate_limit_status);

        let attribution = GitHubRequestAttribution {
            operation_name,
            family,
            resource: Some(resource.clone()),
            graphql_cost,
            remaining: rate_limit_status.remaining,
            limit: rate_limit_status.limit,
            used: rate_limit_status.used,
            spent,
            duration_ms: duration.as_millis().min(u128::from(u64::MAX)) as u64,
        };

        state.latest_rate_limit = Some(rate_limit_status.clone());
        state.latest_rate_limits.insert(resource, rate_limit_status);
        state.latest_request_attribution = Some(attribution.clone());
        state
            .recent_request_attributions
            .push_back(attribution.clone());
        while state.recent_request_attributions.len() > MAX_REQUEST_ATTRIBUTION_HISTORY {
            drop(state.recent_request_attributions.pop_front());
        }

        let graphql_expense = attribution.graphql_cost.or(attribution.spent);
        if attribution.family == GitHubApiFamily::GraphQl && graphql_expense.unwrap_or(0) >= 20 {
            tracing::warn!(
                operation = attribution.operation_name,
                graphql_cost = attribution.graphql_cost,
                spent = attribution.spent,
                remaining = attribution.remaining,
                limit = attribution.limit,
                duration_ms = attribution.duration_ms,
                "expensive github graphql request completed"
            );
        }

        if attribution.family == GitHubApiFamily::GraphQl {
            tracing::info!(
                operation = attribution.operation_name,
                graphql_cost = attribution.graphql_cost,
                spent = attribution.spent,
                remaining = attribution.remaining,
                limit = attribution.limit,
                duration_ms = attribution.duration_ms,
                "github graphql request completed"
            );
        }

        tracing::debug!(
            operation = attribution.operation_name,
            family = attribution.family.label(),
            resource = attribution.resource.as_deref(),
            graphql_cost = attribution.graphql_cost,
            spent = attribution.spent,
            remaining = attribution.remaining,
            limit = attribution.limit,
            duration_ms = attribution.duration_ms,
            "github request completed"
        );
    }
}

fn rate_limit_spent(
    previous: Option<&GitHubRateLimitStatus>,
    current: &GitHubRateLimitStatus,
) -> Option<u64> {
    if let (Some(previous), Some(current_used)) = (previous, current.used)
        && let Some(previous_used) = previous.used
    {
        return current_used.checked_sub(previous_used);
    }

    if let (Some(previous), Some(current_remaining)) = (previous, current.remaining)
        && let Some(previous_remaining) = previous.remaining
    {
        return previous_remaining.checked_sub(current_remaining);
    }

    None
}
