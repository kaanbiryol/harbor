use std::time::{SystemTime, UNIX_EPOCH};

use harbor_github::GitHubRateLimitStatus;

use crate::visual::color;

pub(super) fn github_rate_limit_label(rate_limit: &GitHubRateLimitStatus) -> Option<String> {
    let resource = rate_limit.resource.as_deref().unwrap_or("api");
    let budget = match (rate_limit.remaining, rate_limit.limit) {
        (Some(remaining), Some(limit)) => format!("{remaining}/{limit}"),
        (Some(remaining), None) => format!("{remaining} left"),
        (None, Some(limit)) => format!("limit {limit}"),
        (None, None) => return None,
    };

    if github_rate_limit_should_warn(rate_limit) {
        if let Some(retry_after_seconds) = rate_limit.retry_after_seconds {
            return Some(format!(
                "github {resource}: {budget} retry {}",
                duration_label(retry_after_seconds)
            ));
        }

        if let Some(reset_label) = rate_limit.reset_epoch_seconds.and_then(reset_epoch_label) {
            return Some(format!("github {resource}: {budget} resets {reset_label}"));
        }
    }

    Some(format!("github {resource}: {budget}"))
}

pub(super) fn github_rate_limits_label(rate_limits: &[GitHubRateLimitStatus]) -> Option<String> {
    if rate_limits.len() <= 1 {
        return rate_limits.first().and_then(github_rate_limit_label);
    }

    let labels = rate_limits
        .iter()
        .filter_map(|rate_limit| {
            let resource = rate_limit.resource.as_deref().unwrap_or("api");
            match (rate_limit.remaining, rate_limit.limit) {
                (Some(remaining), Some(limit)) => Some(format!("{resource} {remaining}/{limit}")),
                (Some(remaining), None) => Some(format!("{resource} {remaining} left")),
                _ => None,
            }
        })
        .collect::<Vec<_>>();

    (!labels.is_empty()).then(|| format!("github {}", labels.join(" ")))
}

pub(super) fn github_rate_limit_color(rate_limit: &GitHubRateLimitStatus) -> gpui::Rgba {
    if rate_limit.remaining == Some(0) {
        color::danger()
    } else if github_rate_limit_should_warn(rate_limit) {
        color::warning()
    } else {
        color::text_muted()
    }
}

fn github_rate_limit_should_warn(rate_limit: &GitHubRateLimitStatus) -> bool {
    match (rate_limit.remaining, rate_limit.limit) {
        (Some(remaining), Some(limit)) if limit > 0 => remaining.saturating_mul(5) <= limit,
        (Some(remaining), _) => remaining <= 100,
        _ => false,
    }
}

fn reset_epoch_label(epoch_seconds: u64) -> Option<String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();

    if epoch_seconds <= now {
        return Some("now".to_string());
    }

    Some(duration_label(epoch_seconds - now))
}

fn duration_label(seconds: u64) -> String {
    if seconds < 60 {
        format!("in {seconds}s")
    } else if seconds < 3600 {
        format!("in {}m", seconds.div_ceil(60))
    } else {
        format!("in {}h", seconds.div_ceil(3600))
    }
}
