// Copyright (c) 2021  Teddy Wing
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


use rusqlite::{self, OptionalExtension};
use thiserror;

use crate::github;


/// Repository metadata mapped to the database.
#[derive(Debug)]
pub struct Repo {
    id: i64,
    name: Option<String>,
    description: Option<String>,
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
        Self {
            id: repo.id,
            name: Some(repo.name.clone()),
            description: repo.description.clone(),
            updated_at: Some(repo.updated_at.clone()),
        }
    }
}


#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error")]
    Db(#[from] rusqlite::Error),
}


#[derive(Debug)]
pub struct Db {
    connection: rusqlite::Connection,
}

impl Db {
    /// Open a connection to the database.
    pub fn connect(path: &str) -> Result<Self, Error> {
        Ok(
            Db {
                connection: rusqlite::Connection::open_with_flags(
                    path,
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                    | rusqlite::OpenFlags::SQLITE_OPEN_CREATE,
                )?,
            }
        )
    }

    /// Initialise the database with tables and indexes.
    pub fn create(&mut self) -> Result<(), Error> {
        let tx = self.connection.transaction()?;

        tx.execute(
            r#"
                CREATE TABLE IF NOT EXISTS repositories (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    description TEXT,
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
    pub fn repo_get(&mut self, id: i64) -> Result<Repo, Error> {
        let tx = self.connection.transaction()?;

        let repo = tx.query_row(
            r#"
            SELECT
                id,
                name,
                description,
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
                        updated_at: Some(row.get(3)?),
                    }
                )
            },
        )?;

        tx.commit()?;

        Ok(repo)
    }

    /// Insert a new repository.
    pub fn repo_insert(&mut self, repo: Repo) -> Result<(), Error> {
        let tx = self.connection.transaction()?;

        tx.execute(
            r#"
            INSERT INTO repositories
                (id, name, description, updated_at)
                VALUES
                (?, ?, ?, ?)
            "#,
            rusqlite::params![
                repo.id,
                &repo.name,
                &repo.description,
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
        &mut self,
        repo: &Repo,
    ) -> Result<bool, Error> {
        let tx = self.connection.transaction()?;

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
    pub fn repo_update(&mut self, repo: &Repo) -> Result<(), Error> {
        let tx = self.connection.transaction()?;

        tx.execute(
            r#"
            UPDATE repositories
            SET
                name = ?,
                description = ?,
                updated_at = ?
            WHERE id = ?
            "#,
            rusqlite::params![
                &repo.name,
                &repo.description,
                &repo.updated_at,
                repo.id,
            ],
        )?;

        tx.commit()?;

        Ok(())
    }
}
