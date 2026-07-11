pub(crate) fn avatar_url(login: &str) -> Option<String> {
    let login = login.trim();

    if login.is_empty()
        || login.eq_ignore_ascii_case("ghost")
        || login.eq_ignore_ascii_case("you")
        || login.chars().any(char::is_whitespace)
    {
        None
    } else {
        Some(format!("https://github.com/{login}.png?size=48"))
    }
}

pub(crate) fn profile_url(login: &str) -> String {
    format!("https://github.com/{login}")
}

pub(crate) fn avatar_initial(label: &str) -> String {
    label
        .trim()
        .chars()
        .find(|character| character.is_alphanumeric())
        .map(|character| character.to_uppercase().collect())
        .unwrap_or_else(|| "?".to_string())
}

#[cfg(test)]
mod tests {
    use super::{avatar_initial, avatar_url, profile_url};

    #[test]
    fn builds_avatar_urls_for_github_users() {
        assert_eq!(
            avatar_url("octocat").as_deref(),
            Some("https://github.com/octocat.png?size=48")
        );
        assert_eq!(avatar_url("ghost"), None);
        assert_eq!(avatar_url("you"), None);
        assert_eq!(avatar_url("bad login"), None);
        assert_eq!(avatar_url(""), None);
    }

    #[test]
    fn builds_profile_urls() {
        assert_eq!(profile_url("octocat"), "https://github.com/octocat");
    }

    #[test]
    fn derives_avatar_initials() {
        assert_eq!(avatar_initial("octocat"), "O");
        assert_eq!(avatar_initial(" team-reviewers"), "T");
        assert_eq!(avatar_initial(""), "?");
    }
}
