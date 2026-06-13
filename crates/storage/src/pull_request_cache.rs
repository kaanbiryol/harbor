use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, RepoId, ReviewThread, WorkflowRun,
};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{Row, Sqlite, Transaction};

use super::{PullRequestDetailSection, Result, SqliteStore, StorageError};

impl SqliteStore {
    pub async fn load_pull_request_inbox(
        &self,
        repository: &RepoId,
        mode: &str,
    ) -> Result<Vec<PullRequest>> {
        let rows = sqlx::query(
            "SELECT pr_json
             FROM pull_request_inbox_cache
             WHERE owner = ?1 AND name = ?2 AND mode = ?3
             ORDER BY position ASC",
        )
        .bind(&repository.owner)
        .bind(&repository.name)
        .bind(mode)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let json = row.get::<String, _>("pr_json");
                serde_json::from_str(&json).map_err(StorageError::from)
            })
            .collect()
    }

    pub async fn save_pull_request_inbox(
        &self,
        repository: &RepoId,
        mode: &str,
        pull_requests: &[PullRequest],
    ) -> Result<()> {
        let mut transaction = self.pool.begin().await?;

        sqlx::query(
            "DELETE FROM pull_request_inbox_cache
             WHERE owner = ?1 AND name = ?2 AND mode = ?3",
        )
        .bind(&repository.owner)
        .bind(&repository.name)
        .bind(mode)
        .execute(&mut *transaction)
        .await?;

        for (position, pull_request) in pull_requests.iter().enumerate() {
            sqlx::query(
                "INSERT INTO pull_request_inbox_cache
                    (owner, name, mode, number, head_sha, position, pr_json, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, unixepoch())",
            )
            .bind(&repository.owner)
            .bind(&repository.name)
            .bind(mode)
            .bind(pull_request.number as i64)
            .bind(&pull_request.head_sha)
            .bind(position as i64)
            .bind(serde_json::to_string(pull_request)?)
            .execute(&mut *transaction)
            .await?;
        }

        Self::record_sync_success_in_transaction(
            &mut transaction,
            &inbox_target_key(repository, mode),
        )
        .await?;
        transaction.commit().await?;

        Ok(())
    }

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
    ) -> Result<Option<(Vec<PullRequestReview>, Vec<ReviewThread>)>> {
        let reviews = self
            .load_pull_request_detail_section::<Vec<PullRequestReview>>(
                repository,
                number,
                head_sha,
                PullRequestDetailSection::Reviews,
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

        Ok(match (reviews, threads) {
            (Some(reviews), Some(threads)) => Some((reviews, threads)),
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

        row.map(|row| {
            let json = row.get::<String, _>("data_json");
            serde_json::from_str(&json).map_err(StorageError::from)
        })
        .transpose()
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
