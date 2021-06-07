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


use reqwest::ClientBuilder;
use serde::Deserialize;
use thiserror;


const USER_AGENT: &'static str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
);


#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("request error")]
    Http(#[from] reqwest::Error),

    #[error("request header error")]
    Header(#[from] reqwest::header::InvalidHeaderValue),
}


#[derive(Debug, Deserialize)]
pub struct Repo {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub fork: bool,
    pub git_url: String,
    pub default_branch: String,
    pub size: u64,
    pub updated_at: String,
}

impl Repo {
    /// Get the repository description or an empty string if `None`.
    pub fn description(&self) -> &str {
        self.description
            .as_deref()
            .unwrap_or("")
    }
}


/// Fetch all GitHub repositories for the given user.
pub async fn fetch_repos(github_username: &str) -> Result<Vec<Repo>, Error> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Accept", "application/vnd.github.v3+json".parse()?);

    let client = ClientBuilder::new()
        .user_agent(USER_AGENT)
        .default_headers(headers)
        .build()?;

    let mut repos = Vec::new();

    for i in 1.. {
        let repo_page = client.request(
            reqwest::Method::GET,
            format!(
                "https://api.github.com/users/{}/repos?page={}&per_page=100&sort=updated",
                github_username,
                i,
            ),
        )
            .send()
            .await?
            .json::<Vec<Repo>>()
            .await?;

        if repo_page.is_empty() {
            break;
        }

        repos.extend(repo_page);
    }

    Ok(repos)
}
