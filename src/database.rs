use sqlx::{self, ConnectOptions, Connection, Executor};

use crate::github::Repo;


pub struct Db {
    connection: sqlx::SqliteConnection,
}

impl Db {
    pub async fn connect(
        path: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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

    pub async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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

    pub fn repo_insert(repo: &Repo) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    pub fn repo_is_updated() -> Result<bool, Box<dyn std::error::Error>> {
        Ok(false)
    }

    pub fn repo_update(repo: &Repo) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
