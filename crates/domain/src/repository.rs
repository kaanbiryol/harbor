use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct RepoId {
    pub owner: String,
    pub name: String,
}

impl RepoId {
    pub fn new(owner: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        let (owner, name) = value.split_once('/')?;

        if owner.is_empty()
            || name.is_empty()
            || name.contains('/')
            || owner.chars().any(char::is_whitespace)
            || name.chars().any(char::is_whitespace)
        {
            None
        } else {
            Some(Self::new(owner, name))
        }
    }

    pub fn matches_ignore_ascii_case(&self, other: &Self) -> bool {
        self.owner.eq_ignore_ascii_case(&other.owner) && self.name.eq_ignore_ascii_case(&other.name)
    }

    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Repository {
    pub id: RepoId,
    pub default_branch: String,
    pub private: bool,
}
