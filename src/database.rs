// Copyright (c) 2021, 2022  Teddy Wing
//
// This file is part of Reflectub.
//
// Reflectub is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Reflectub is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Reflectub. If not, see <https://www.gnu.org/licenses/>.


use r2d2;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{self, OptionalExtension};
use thiserror;

use crate::github;


/// Repository metadata mapped to the database.
#[derive(Debug)]
pub struct Repo {
    id: i64,
    name: Option<String>,
    description: Option<String>,
    pub default_branch: Option<String>,
    updated_at: Option<String>,
}

impl Repo {
    pub fn description(&self) -> &str {
        self.description
            .as_deref()
            .unwrap_or("")
    }
}

impl From<&github::Repo> for Repo {
    fn from(repo: &github::Repo) -> Self {
        use chrono::DateTime;

        let repo_updated_at = DateTime::parse_from_rfc3339(&repo.updated_at).ok();
        let repo_pushed_at = DateTime::parse_from_rfc3339(&repo.pushed_at).ok();

        // Set `updated_at` to the most recent of `repo_updated_at` or
        // `repo_pushed_at`.
        let updated_at =
            if repo_updated_at.is_none() && repo_pushed_at.is_none() {
                repo.updated_at.clone()

            // `repo_updated_at` and `repo_pushed_at` are both Some.
            } else if repo_pushed_at.unwrap() > repo_updated_at.unwrap() {
                repo.pushed_at.clone()

            // Default to `repo.updated_at`.
            } else {
                repo.updated_at.clone()
            };

        Self {
            id: repo.id,
            name: Some(repo.name.clone()),
            description: repo.description.clone(),
            default_branch: Some(repo.default_branch.clone()),
            updated_at: Some(updated_at),
        }
    }
}


#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error")]
    Db(#[from] rusqlite::Error),

    #[error("connection pool error")]
    Pool(#[from] r2d2::Error),
}


#[derive(Debug)]
pub struct Db {
    pool: r2d2::Pool<SqliteConnectionManager>,
}

impl Db {
    /// Open a connection to the database.
    pub fn connect(path: &str) -> Result<Self, Error> {
        let manager = SqliteConnectionManager::file(path)
            .with_flags(
                rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                | rusqlite::OpenFlags::SQLITE_OPEN_CREATE,
            );

        Ok(
            Db {
                pool: r2d2::Pool::new(manager)?,
            }
        )
    }

    /// Initialise the database with tables and indexes.
    pub fn create(&self) -> Result<(), Error> {
        let mut pool = self.pool.get()?;
        let tx = pool.transaction()?;

        tx.execute(
            r#"
                CREATE TABLE IF NOT EXISTS repositories (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    description TEXT,
                    default_branch TEXT,
                    updated_at TEXT NOT NULL
                );
            "#,
            [],
        )?;

        tx.execute(
            r#"
                CREATE UNIQUE INDEX IF NOT EXISTS idx_repositories_id
                    ON repositories (id);
            "#,
            [],
        )?;

        tx.commit()?;

        Ok(())
    }

    /// Get a repository by its ID.
    ///
    /// Returns a `rusqlite::Error::QueryReturnedNoRows` error if the row
    /// doesn't exist.
    pub fn repo_get(&self, id: i64) -> Result<Repo, Error> {
        let mut pool = self.pool.get()?;
        let tx = pool.transaction()?;

        let repo = tx.query_row(
            r#"
            SELECT
                id,
                name,
                description,
                default_branch,
                updated_at
            FROM repositories
            WHERE id = ?
            "#,
            [id],
            |row| {
                Ok(
                    Repo {
                        id: row.get(0)?,
                        name: Some(row.get(1)?),
                        description: row.get(2)?,
                        default_branch: row.get(3)?,
                        updated_at: Some(row.get(4)?),
                    }
                )
            },
        )?;

        tx.commit()?;

        Ok(repo)
    }

    /// Insert a new repository.
    pub fn repo_insert(&self, repo: Repo) -> Result<(), Error> {
        let mut pool = self.pool.get()?;
        let tx = pool.transaction()?;

        tx.execute(
            r#"
            INSERT INTO repositories
                (id, name, description, default_branch, updated_at)
                VALUES
                (?, ?, ?, ?, ?)
            "#,
            rusqlite::params![
                repo.id,
                &repo.name,
                &repo.description,
                &repo.default_branch,
                &repo.updated_at,
            ],
        )?;

        tx.commit()?;

        Ok(())
    }

    /// Check if the given repository is newer than the one in the repository.
    ///
    /// Compares the `updated_at` field to find out whether the repository was
    /// updated.
    pub fn repo_is_updated(
        &self,
        repo: &Repo,
    ) -> Result<bool, Error> {
        let mut pool = self.pool.get()?;
        let tx = pool.transaction()?;

        let is_updated = match tx.query_row(
            r#"
            SELECT 1
            FROM repositories
            WHERE id = ?
                AND datetime(updated_at) < datetime(?)
            "#,
            rusqlite::params![
                repo.id,
                &repo.updated_at,
            ],
            |row| row.get::<usize, u8>(0),
        )
            .optional()
        {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(e.into()),
        };

        tx.commit()?;

        is_updated
    }

    /// Update an existing repository.
    pub fn repo_update(&self, repo: &Repo) -> Result<(), Error> {
        let mut pool = self.pool.get()?;
        let tx = pool.transaction()?;

        tx.execute(
            r#"
            UPDATE repositories
            SET
                name = ?,
                description = ?,
                default_branch = ?,
                updated_at = ?
            WHERE id = ?
            "#,
            rusqlite::params![
                &repo.name,
                &repo.description,
                &repo.default_branch,
                &repo.updated_at,
                repo.id,
            ],
        )?;

        tx.commit()?;

        Ok(())
    }
}
