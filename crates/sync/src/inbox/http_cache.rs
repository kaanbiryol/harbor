use harbor_domain::RepoId;
use harbor_github::HttpCacheValidator;
use harbor_storage::StoredHttpCacheValidator;

use super::PullRequestInboxMode;

pub(super) fn http_validator_key(repository: &RepoId, mode: PullRequestInboxMode) -> String {
    format!("rest-inbox:{}:{}", repository.full_name(), mode.key())
}

pub(super) fn github_validator_from_storage(
    validator: StoredHttpCacheValidator,
) -> HttpCacheValidator {
    HttpCacheValidator {
        etag: validator.etag,
        last_modified: validator.last_modified,
    }
}

pub(super) fn storage_validator_from_github(
    validator: HttpCacheValidator,
) -> StoredHttpCacheValidator {
    StoredHttpCacheValidator {
        etag: validator.etag,
        last_modified: validator.last_modified,
    }
}
