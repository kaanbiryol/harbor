#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitHubAuthSource {
    OAuth,
    GhCli,
}

impl GitHubAuthSource {
    pub(crate) fn storage_value(self) -> &'static str {
        match self {
            Self::OAuth => "oauth",
            Self::GhCli => "gh_cli",
        }
    }

    pub(super) fn from_storage_value(value: &str) -> Option<Self> {
        match value.trim() {
            "oauth" => Some(Self::OAuth),
            "gh_cli" => Some(Self::GhCli),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::OAuth => "GitHub OAuth",
            Self::GhCli => "GitHub CLI",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GitHubCliAvailability {
    Checking,
    Available,
    Unavailable(String),
}

impl GitHubCliAvailability {
    pub(crate) fn unavailable_reason(&self) -> Option<&str> {
        match self {
            Self::Checking => Some("Checking GitHub CLI..."),
            Self::Available => None,
            Self::Unavailable(reason) => Some(reason.as_str()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GitHubAuthStatus {
    Loading,
    SignedOut,
    MissingClientId,
    SigningIn {
        user_code: String,
        verification_uri: String,
    },
    SignedIn {
        login: Option<String>,
        source: GitHubAuthSource,
    },
    Failed(String),
}

impl GitHubAuthStatus {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Loading => "GitHub",
            Self::SignedOut | Self::MissingClientId | Self::Failed(_) => "Sign in",
            Self::SigningIn { .. } => "Waiting",
            Self::SignedIn { .. } => "GitHub",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_source_round_trips_storage_values() {
        assert_eq!(
            GitHubAuthSource::from_storage_value(GitHubAuthSource::OAuth.storage_value()),
            Some(GitHubAuthSource::OAuth)
        );
        assert_eq!(
            GitHubAuthSource::from_storage_value(GitHubAuthSource::GhCli.storage_value()),
            Some(GitHubAuthSource::GhCli)
        );
    }

    #[test]
    fn auth_source_rejects_unknown_storage_values() {
        assert_eq!(GitHubAuthSource::from_storage_value(""), None);
        assert_eq!(GitHubAuthSource::from_storage_value("github"), None);
    }
}
