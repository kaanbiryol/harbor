use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestComment, PullRequestReview, RepoId, ReviewThread,
    WorkflowRun,
};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{Row, Sqlite, Transaction};

use super::{PullRequestDetailSection, Result, SqliteStore};

impl SqliteStore {
    pub async fn save_pull_request_metadata(&self, pull_request: &PullRequest) -> Result<()> {
        self.save_pull_request_detail_section(
            &pull_request.repo,
            pull_request.number,
            &pull_request.head_sha,
            PullRequestDetailSection::Metadata,
            pull_request,
        )
        .await
    }

    pub async fn load_pull_request_metadata(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
    ) -> Result<Option<PullRequest>> {
        self.load_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::Metadata,
        )
        .await
    }

    pub async fn save_pull_request_files(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        files: &[DiffFile],
    ) -> Result<()> {
        self.save_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::Files,
            files,
        )
        .await
    }

    pub async fn load_pull_request_files(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
    ) -> Result<Option<Vec<DiffFile>>> {
        self.load_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::Files,
        )
        .await
    }

    pub async fn save_pull_request_reviews(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        reviews: &[PullRequestReview],
        comments: &[PullRequestComment],
        threads: &[ReviewThread],
    ) -> Result<()> {
        let mut transaction = self.pool.begin().await?;

        Self::save_pull_request_detail_section_in_transaction(
            &mut transaction,
            repository,
            number,
            head_sha,
            PullRequestDetailSection::Reviews,
            reviews,
        )
        .await?;
        Self::record_sync_success_in_transaction(
            &mut transaction,
            &detail_target_key(repository, number, PullRequestDetailSection::Reviews),
        )
        .await?;
        Self::save_pull_request_detail_section_in_transaction(
            &mut transaction,
            repository,
            number,
            head_sha,
            PullRequestDetailSection::PullRequestComments,
            comments,
        )
        .await?;
        Self::record_sync_success_in_transaction(
            &mut transaction,
            &detail_target_key(
                repository,
                number,
                PullRequestDetailSection::PullRequestComments,
            ),
        )
        .await?;
        Self::save_pull_request_detail_section_in_transaction(
            &mut transaction,
            repository,
            number,
            head_sha,
            PullRequestDetailSection::ReviewThreads,
            threads,
        )
        .await?;
        Self::record_sync_success_in_transaction(
            &mut transaction,
            &detail_target_key(repository, number, PullRequestDetailSection::ReviewThreads),
        )
        .await?;
        transaction.commit().await?;

        Ok(())
    }

    pub async fn load_pull_request_reviews(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
    ) -> Result<
        Option<(
            Vec<PullRequestReview>,
            Vec<PullRequestComment>,
            Vec<ReviewThread>,
        )>,
    > {
        let reviews = self
            .load_pull_request_detail_section::<Vec<PullRequestReview>>(
                repository,
                number,
                head_sha,
                PullRequestDetailSection::Reviews,
            )
            .await?;
        let comments = self
            .load_pull_request_detail_section::<Vec<PullRequestComment>>(
                repository,
                number,
                head_sha,
                PullRequestDetailSection::PullRequestComments,
            )
            .await?;
        let threads = self
            .load_pull_request_detail_section::<Vec<ReviewThread>>(
                repository,
                number,
                head_sha,
                PullRequestDetailSection::ReviewThreads,
            )
            .await?;

        Ok(match (reviews, comments, threads) {
            (Some(reviews), Some(comments), Some(threads)) => Some((reviews, comments, threads)),
            _ => None,
        })
    }

    pub async fn save_pull_request_check_runs(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        check_runs: &[CheckRun],
    ) -> Result<()> {
        self.save_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::CheckRuns,
            check_runs,
        )
        .await
    }

    pub async fn load_pull_request_check_runs(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
    ) -> Result<Option<Vec<CheckRun>>> {
        self.load_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::CheckRuns,
        )
        .await
    }

    pub async fn save_pull_request_workflow_runs(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        workflow_runs: &[WorkflowRun],
    ) -> Result<()> {
        self.save_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::WorkflowRuns,
            workflow_runs,
        )
        .await
    }

    pub async fn load_pull_request_workflow_runs(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
    ) -> Result<Option<Vec<WorkflowRun>>> {
        self.load_pull_request_detail_section(
            repository,
            number,
            head_sha,
            PullRequestDetailSection::WorkflowRuns,
        )
        .await
    }

    async fn save_pull_request_detail_section<T>(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        section: PullRequestDetailSection,
        value: &T,
    ) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        let mut transaction = self.pool.begin().await?;
        Self::save_pull_request_detail_section_in_transaction(
            &mut transaction,
            repository,
            number,
            head_sha,
            section,
            value,
        )
        .await?;
        Self::record_sync_success_in_transaction(
            &mut transaction,
            &detail_target_key(repository, number, section),
        )
        .await?;
        transaction.commit().await?;

        Ok(())
    }

    async fn save_pull_request_detail_section_in_transaction<T>(
        transaction: &mut Transaction<'_, Sqlite>,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        section: PullRequestDetailSection,
        value: &T,
    ) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        sqlx::query(
            "INSERT INTO pull_request_detail_cache
                (owner, name, number, head_sha, section, data_json, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, unixepoch())
             ON CONFLICT(owner, name, number, head_sha, section) DO UPDATE SET
                data_json = excluded.data_json,
                fetched_at = unixepoch()",
        )
        .bind(&repository.owner)
        .bind(&repository.name)
        .bind(number as i64)
        .bind(head_sha)
        .bind(section.key())
        .bind(serde_json::to_string(value)?)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }

    async fn load_pull_request_detail_section<T>(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        section: PullRequestDetailSection,
    ) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let row = sqlx::query(
            "SELECT data_json
             FROM pull_request_detail_cache
             WHERE owner = ?1 AND name = ?2 AND number = ?3 AND head_sha = ?4 AND section = ?5",
        )
        .bind(&repository.owner)
        .bind(&repository.name)
        .bind(number as i64)
        .bind(head_sha)
        .bind(section.key())
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let json = row.get::<String, _>("data_json");
        match serde_json::from_str(&json) {
            Ok(value) => Ok(Some(value)),
            Err(error) => {
                tracing::warn!(
                    repository = %repository.full_name(),
                    number,
                    section = section.key(),
                    %error,
                    "discarding invalid pull request detail cache row"
                );
                sqlx::query(
                    "DELETE FROM pull_request_detail_cache
                     WHERE owner = ?1 AND name = ?2 AND number = ?3 AND head_sha = ?4 AND section = ?5",
                )
                .bind(&repository.owner)
                .bind(&repository.name)
                .bind(number as i64)
                .bind(head_sha)
                .bind(section.key())
                .execute(&self.pool)
                .await?;
                Ok(None)
            }
        }
    }
}

pub fn inbox_target_key(repository: &RepoId, mode: &str) -> String {
    format!("inbox:{}:{}", repository.full_name(), mode)
}

pub fn detail_target_key(
    repository: &RepoId,
    number: u64,
    section: PullRequestDetailSection,
) -> String {
    format!("pr:{}#{}:{}", repository.full_name(), number, section.key())
}
