use sqlx::{self, ConnectOptions, Connection, Executor, Row};
use thiserror;

use crate::github;


#[derive(Debug)]
pub struct Repo {
    id: i64,
    name: Option<String>,
    updated_at: Option<String>,
}

impl From<github::Repo> for Repo {
    fn from(repo: github::Repo) -> Self {
        Self {
            id: repo.id,
            name: Some(repo.name),
            updated_at: Some(repo.updated_at),
        }
    }
}


#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error")]
    Db(#[from] sqlx::Error),
}


#[derive(Debug)]
pub struct Db {
    connection: sqlx::SqliteConnection,
}

impl Db {
    pub async fn connect(path: &str) -> Result<Self, Error> {
        Ok(
            Db {
                connection: sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true)
                    .connect()
                    .await?,
            }
        )
    }

    pub async fn create(&mut self) -> Result<(), Error> {
        let mut tx = self.connection.begin().await?;

        tx.execute(r#"
            CREATE TABLE repositories (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
        "#).await?;

        tx.execute(r#"
            CREATE UNIQUE INDEX idx_repositories_id
                ON repositories (id);
        "#).await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn repo_get(&mut self, id: i64) -> Result<Repo, Error> {
        let mut tx = self.connection.begin().await?;

        // NOTE: Returns `RowNotFound` if not found.
        let row = sqlx::query(r#"
            SELECT id, name, updated_at
            FROM repositories
            WHERE id = ?
        "#)
            .bind(id)
            .fetch_one(&mut tx)
            .await?;

        tx.commit().await?;

        Ok(
            Repo {
                id: row.get(0),
                name: Some(row.get(1)),
                updated_at: Some(row.get(2)),
            }
        )
    }

    pub async fn repo_insert(&mut self, repo: Repo) -> Result<(), Error> {
        let mut tx = self.connection.begin().await?;

        sqlx::query(r#"
            INSERT INTO repositories
                (id, name, updated_at)
                VALUES
                (?, ?, ?)
        "#)
            .bind(repo.id)
            .bind(&repo.name)
            .bind(&repo.updated_at)
            .execute(&mut tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn repo_is_updated(
        &mut self,
        repo: &Repo,
    ) -> Result<bool, Error> {
        let mut tx = self.connection.begin().await?;

        let is_updated = match sqlx::query(r#"
            SELECT 1
            FROM repositories
            WHERE id = ?
                AND datetime(updated_at) < datetime(?)
        "#)
            .bind(repo.id)
            .bind(&repo.updated_at)
            .fetch_optional(&mut tx)
            .await
        {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(e.into()),
        };

        tx.commit().await?;

        is_updated
    }

    pub async fn repo_update(&mut self, repo: &Repo) -> Result<(), Error> {
        let mut tx = self.connection.begin().await?;

        sqlx::query(r#"
            UPDATE repositories
            SET updated_at = ?
            WHERE id = ?
        "#)
            .bind(&repo.updated_at)
            .bind(repo.id)
            .execute(&mut tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }
}
