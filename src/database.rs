use sqlx::{self, Connection};

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
                connection: sqlx::SqliteConnection::connect(path)
                    .await?,
            }
        )
    }

    pub fn create() -> Result<(), Box<dyn std::error::Error>> {
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
