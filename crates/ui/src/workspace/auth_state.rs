#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GitHubAuthSource {
    OAuth,
    GhCli,
}

impl GitHubAuthSource {
    pub(crate) fn credential_value(self) -> &'static str {
        match self {
            Self::OAuth => "oauth",
            Self::GhCli => "gh_cli",
        }
    }

    pub(super) fn from_credential_value(value: &str) -> Option<Self> {
        match value.trim() {
            "oauth" => Some(Self::OAuth),
            "gh_cli" => Some(Self::GhCli),
            "token" => Some(Self::OAuth),
            _ => None,
        }
    }

    pub(super) fn from_credential_bytes(
        bytes: Vec<u8>,
    ) -> std::result::Result<Option<Self>, String> {
        let value = String::from_utf8(bytes).map_err(|error| error.to_string())?;

        Ok(Self::from_credential_value(&value))
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
    fn auth_source_round_trips_credential_values() {
        assert_eq!(
            GitHubAuthSource::from_credential_value(GitHubAuthSource::OAuth.credential_value()),
            Some(GitHubAuthSource::OAuth)
        );
        assert_eq!(
            GitHubAuthSource::from_credential_value(GitHubAuthSource::GhCli.credential_value()),
            Some(GitHubAuthSource::GhCli)
        );
    }

    #[test]
    fn auth_source_rejects_unknown_credential_values() {
        assert_eq!(GitHubAuthSource::from_credential_value(""), None);
        assert_eq!(GitHubAuthSource::from_credential_value("github"), None);
    }

    #[test]
    fn legacy_token_auth_source_reads_as_oauth() {
        assert_eq!(
            GitHubAuthSource::from_credential_value("token"),
            Some(GitHubAuthSource::OAuth)
        );
    }
}
