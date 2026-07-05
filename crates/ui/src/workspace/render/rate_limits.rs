use gpui::Hsla;
use harbor_github::GitHubRateLimitStatus;

use crate::visual::color;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct GitHubRateLimitIndicator {
    pub(super) value: f32,
    pub(super) tone: GitHubRateLimitTone,
    pub(super) details: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GitHubRateLimitTone {
    Neutral,
    Warning,
    Danger,
}

pub(super) fn github_rate_limit_indicator(
    rate_limits: &[GitHubRateLimitStatus],
    fallback_rate_limit: Option<&GitHubRateLimitStatus>,
) -> Option<GitHubRateLimitIndicator> {
    let rate_limits = if rate_limits.is_empty() {
        fallback_rate_limit.into_iter().collect::<Vec<_>>()
    } else {
        rate_limits.iter().collect::<Vec<_>>()
    };

    let value = rate_limits
        .iter()
        .filter_map(|rate_limit| github_rate_limit_remaining_percentage(rate_limit))
        .min_by(|left, right| left.total_cmp(right))?;
    let details = rate_limits
        .iter()
        .filter_map(|rate_limit| github_rate_limit_detail(rate_limit))
        .collect::<Vec<_>>();

    Some(GitHubRateLimitIndicator {
        value,
        tone: github_rate_limit_tone(value),
        details,
    })
}

pub(super) fn github_rate_limit_indicator_color(tone: GitHubRateLimitTone) -> Hsla {
    let color: Hsla = match tone {
        GitHubRateLimitTone::Neutral => color::text_muted(),
        GitHubRateLimitTone::Warning => color::warning(),
        GitHubRateLimitTone::Danger => color::danger(),
    }
    .into();

    color.alpha(0.72)
}

fn github_rate_limit_detail(rate_limit: &GitHubRateLimitStatus) -> Option<String> {
    let resource = rate_limit.resource.as_deref().unwrap_or("api");
    match (rate_limit.remaining, rate_limit.limit) {
        (Some(remaining), Some(limit)) => Some(format!("github {resource} {remaining}/{limit}")),
        (Some(remaining), None) => Some(format!("github {resource} {remaining} left")),
        (None, Some(limit)) => Some(format!("github {resource} limit {limit}")),
        (None, None) => None,
    }
}

fn github_rate_limit_remaining_percentage(rate_limit: &GitHubRateLimitStatus) -> Option<f32> {
    let (Some(remaining), Some(limit)) = (rate_limit.remaining, rate_limit.limit) else {
        return None;
    };
    if limit == 0 {
        return None;
    }

    Some((remaining as f32 / limit as f32 * 100.).clamp(0., 100.))
}

fn github_rate_limit_tone(value: f32) -> GitHubRateLimitTone {
    if value <= 5. {
        GitHubRateLimitTone::Danger
    } else if value <= 20. {
        GitHubRateLimitTone::Warning
    } else {
        GitHubRateLimitTone::Neutral
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rate_limit(resource: &str, remaining: u64, limit: u64) -> GitHubRateLimitStatus {
        GitHubRateLimitStatus {
            resource: Some(resource.to_string()),
            remaining: Some(remaining),
            limit: Some(limit),
            ..GitHubRateLimitStatus::default()
        }
    }

    #[test]
    fn indicator_uses_lowest_remaining_percentage() {
        let indicator = github_rate_limit_indicator(
            &[
                rate_limit("core", 4_900, 5_000),
                rate_limit("graphql", 750, 5_000),
            ],
            None,
        )
        .unwrap();

        assert!((indicator.value - 15.).abs() < 0.001);
        assert_eq!(indicator.tone, GitHubRateLimitTone::Warning);
        assert_eq!(
            indicator.details,
            vec!["github core 4900/5000", "github graphql 750/5000"]
        );
    }

    #[test]
    fn indicator_falls_back_to_latest_rate_limit() {
        let fallback = rate_limit("core", 5, 5_000);
        let indicator = github_rate_limit_indicator(&[], Some(&fallback)).unwrap();

        assert_eq!(indicator.value, 0.1);
        assert_eq!(indicator.tone, GitHubRateLimitTone::Danger);
        assert_eq!(indicator.details, vec!["github core 5/5000"]);
    }
}
