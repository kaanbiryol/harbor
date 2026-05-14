use harbor_domain::{DiffFile, PullRequest, RepoId};
use serde_json::json;

use crate::{
    ConditionalFetch, GitHubError, GitHubTransport, HttpCacheValidator, PullRequestEnrichment,
    PullRequestPage, PullRequestPageCursor, Result, dto,
};

use super::{
    GitHubClient, PullRequestListFilter,
    requests::{
        PULL_REQUEST_ENRICHMENT_QUERY, REPOSITORY_PULL_REQUEST_COUNT_QUERY,
        REPOSITORY_PULL_REQUESTS_QUERY, repository_pull_requests_query,
    },
};

const REPOSITORY_PULL_REQUEST_PAGE_LIMIT: usize = 10;
const PULL_REQUEST_ENRICHMENT_CHUNK_SIZE: usize = 10;

impl<T> GitHubClient<T>
where
    T: GitHubTransport,
{
    pub async fn list_open_pull_requests(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<PullRequest>> {
        let path = format!("/repos/{owner}/{repo}/pulls");
        let response = self
            .transport
            .rest_get(
                &path,
                &[("state", "open"), ("per_page", "50"), ("sort", "updated")],
            )
            .await?;

        dto::pull_requests_from_value(RepoId::new(owner, repo), response)
    }

    pub async fn list_repository_pull_requests(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
    ) -> Result<Vec<PullRequest>> {
        let mut pull_requests = Vec::new();
        let mut cursor = None;
        let mut pages_loaded = 0;

        loop {
            if pages_loaded >= REPOSITORY_PULL_REQUEST_PAGE_LIMIT {
                return Err(GitHubError::RequestBudget(format!(
                    "stopped loading repository pull requests after {REPOSITORY_PULL_REQUEST_PAGE_LIMIT} pages"
                )));
            }
            pages_loaded += 1;

            let page = self
                .list_repository_pull_request_page(repository, filter, cursor, 100)
                .await?;
            pull_requests.extend(page.pull_requests);

            match page.next_cursor {
                Some(next_cursor) => cursor = Some(next_cursor),
                None => break,
            }
        }

        Ok(pull_requests)
    }

    pub async fn list_repository_pull_request_page(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
        cursor: Option<PullRequestPageCursor>,
        page_size: usize,
    ) -> Result<PullRequestPage> {
        let after = match cursor {
            Some(PullRequestPageCursor::GraphQl(cursor)) => Some(cursor),
            Some(PullRequestPageCursor::RestPage(_)) => {
                return Err(GitHubError::Mapping(
                    "REST pull request page cursor cannot be used for GraphQL search".to_string(),
                ));
            }
            None => None,
        };
        let response = self
            .transport
            .graphql(
                REPOSITORY_PULL_REQUESTS_QUERY,
                json!({
                    "searchQuery": repository_pull_requests_query(repository, filter),
                    "first": page_size.clamp(1, 100),
                    "after": after,
                }),
            )
            .await?;
        let page = dto::pull_request_search_page_from_graphql_value(response)?;
        let next_cursor = if page.has_next_page {
            Some(PullRequestPageCursor::GraphQl(page.end_cursor.ok_or_else(
                || {
                    GitHubError::Mapping(
                        "repository pull request page was missing an end cursor".to_string(),
                    )
                },
            )?))
        } else {
            None
        };

        Ok(PullRequestPage {
            pull_requests: page.pull_requests,
            total_count: page.total_count,
            next_cursor,
        })
    }

    pub async fn count_repository_pull_requests(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
    ) -> Result<usize> {
        let search_query = repository_pull_requests_query(repository, filter);
        let response = self
            .transport
            .graphql(
                REPOSITORY_PULL_REQUEST_COUNT_QUERY,
                json!({
                    "searchQuery": search_query,
                }),
            )
            .await?;

        dto::pull_request_search_count_from_graphql_value(response)
    }

    pub async fn list_repository_pull_requests_light(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
        validator: Option<&HttpCacheValidator>,
    ) -> Result<ConditionalFetch<Vec<PullRequest>>> {
        if filter == PullRequestListFilter::NeedsReview {
            return self
                .list_repository_pull_requests(repository, filter)
                .await
                .map(|pull_requests| ConditionalFetch::Modified {
                    value: pull_requests,
                    validator: None,
                });
        }

        let path = format!("/repos/{}/{}/pulls", repository.owner, repository.name);
        let state = pull_request_rest_state(filter);
        let first_page_query = [
            ("state", state),
            ("per_page", "100"),
            ("sort", "updated"),
            ("direction", "desc"),
        ];
        let first_page = self
            .transport
            .rest_get_conditional(&path, &first_page_query, validator)
            .await?;
        let (mut pull_requests, validator) = match first_page {
            ConditionalFetch::NotModified { validator } => {
                return Ok(ConditionalFetch::NotModified { validator });
            }
            ConditionalFetch::Modified { value, validator } => (
                dto::pull_requests_from_value(repository.clone(), value)?,
                validator,
            ),
        };

        let mut page = 2;
        while pull_requests.len() == (page - 1) * 100 && page <= REPOSITORY_PULL_REQUEST_PAGE_LIMIT
        {
            let page_string = page.to_string();
            let page_query = [
                ("state", state),
                ("per_page", "100"),
                ("sort", "updated"),
                ("direction", "desc"),
                ("page", page_string.as_str()),
            ];
            let value = self.transport.rest_get(&path, &page_query).await?;
            let page_pull_requests = dto::pull_requests_from_value(repository.clone(), value)?;
            let page_count = page_pull_requests.len();
            pull_requests.extend(page_pull_requests);
            if page_count < 100 {
                break;
            }
            page += 1;
        }

        Ok(ConditionalFetch::Modified {
            value: pull_requests,
            validator,
        })
    }

    pub async fn list_repository_pull_requests_light_page(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
        cursor: Option<PullRequestPageCursor>,
        page_size: usize,
        validator: Option<&HttpCacheValidator>,
    ) -> Result<ConditionalFetch<PullRequestPage>> {
        if filter == PullRequestListFilter::NeedsReview {
            return self
                .list_repository_pull_request_page(repository, filter, cursor, page_size)
                .await
                .map(|page| ConditionalFetch::Modified {
                    value: page,
                    validator: None,
                });
        }

        let page = match cursor {
            Some(PullRequestPageCursor::RestPage(page)) => page,
            Some(PullRequestPageCursor::GraphQl(_)) => {
                return Err(GitHubError::Mapping(
                    "GraphQL pull request page cursor cannot be used for REST list".to_string(),
                ));
            }
            None => 1,
        };
        let page_size = page_size.clamp(1, 100);
        let page_size_string = page_size.to_string();
        let page_string = page.to_string();
        let path = format!("/repos/{}/{}/pulls", repository.owner, repository.name);
        let state = pull_request_rest_state(filter);
        let mut query = vec![
            ("state", state),
            ("per_page", page_size_string.as_str()),
            ("sort", "updated"),
            ("direction", "desc"),
        ];
        if page > 1 {
            query.push(("page", page_string.as_str()));
        }

        let fetch = if page == 1 {
            self.transport
                .rest_get_conditional(&path, &query, validator)
                .await?
        } else {
            ConditionalFetch::Modified {
                value: self.transport.rest_get(&path, &query).await?,
                validator: None,
            }
        };

        match fetch {
            ConditionalFetch::NotModified { validator } => {
                Ok(ConditionalFetch::NotModified { validator })
            }
            ConditionalFetch::Modified { value, validator } => {
                let pull_requests = dto::pull_requests_from_value(repository.clone(), value)?;
                let next_cursor = if pull_requests.len() == page_size {
                    Some(PullRequestPageCursor::RestPage(page + 1))
                } else {
                    None
                };
                Ok(ConditionalFetch::Modified {
                    value: PullRequestPage {
                        pull_requests,
                        total_count: None,
                        next_cursor,
                    },
                    validator,
                })
            }
        }
    }

    pub async fn enrich_pull_requests_by_node_ids(
        &self,
        node_ids: &[String],
    ) -> Result<Vec<PullRequestEnrichment>> {
        let mut enrichments = Vec::new();

        for chunk in node_ids.chunks(PULL_REQUEST_ENRICHMENT_CHUNK_SIZE) {
            let response = self
                .transport
                .graphql(PULL_REQUEST_ENRICHMENT_QUERY, json!({ "ids": chunk }))
                .await?;
            enrichments.extend(dto::pull_request_enrichments_from_graphql_value(response)?);
        }

        Ok(enrichments)
    }

    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<PullRequest> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}");
        let response = self.transport.rest_get(&path, &[]).await?;

        dto::pull_request_from_value(RepoId::new(owner, repo), response)
    }

    pub async fn list_pull_request_files(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<DiffFile>> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}/files");
        let response = self
            .transport
            .rest_get(&path, &[("per_page", "100")])
            .await?;

        dto::diff_files_from_value(response)
    }

    pub async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        head_sha: &str,
    ) -> Result<()> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}/merge");
        self.transport
            .rest_put(
                &path,
                json!({
                    "sha": head_sha,
                    "merge_method": "squash",
                }),
            )
            .await?;

        Ok(())
    }
}

fn pull_request_rest_state(filter: PullRequestListFilter) -> &'static str {
    match filter {
        PullRequestListFilter::Open | PullRequestListFilter::NeedsReview => "open",
        PullRequestListFilter::Closed => "closed",
    }
}
