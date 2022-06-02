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


use thiserror;

use std::fs;
use std::io::Write;
use std::path::Path;


#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot create repo '{path}'")]
    MirrorCreateRepo {
        #[from]
        source: git2::Error,
        path: String,
    },
    #[error("")]
    MirrorAddRemote(),
    #[error("")]
    MirrorConfig(#[from] git2::Error),
    #[error("")]
    MirrorRemoteEnableMirror(),
    #[error("")]
    MirrorFetch(),

    #[error("")]
    UpdateOpenRepo(),
    #[error("")]
    UpdateGetRemotes(),
    #[error("")]
    UpdateFindRemote(),
    #[error("")]
    UpdateFetch(),


    #[error("")]
    GitChangeBranch(),

    #[error("git error")]
    Git(#[from] git2::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}


/// Mirror a repository.
///
/// Works like:
///
/// ```shell
/// git clone --mirror URL
/// ```
pub fn mirror<P: AsRef<Path>>(
    url: &str,
    path: P,
    description: &str,
    default_branch: &str,
) -> Result<(), Error> {
    let repo = git2::Repository::init_opts(
        path,
        &git2::RepositoryInitOptions::new()
            .bare(true)

            // On Linux, using the external template prevents the custom
            // description from being added. It doesn't make a difference on
            // Mac OS.
            .external_template(false)
            .description(description),
    )
        .map_err(|e| Error::MirrorCreateRepo{e, path})?;

    let remote_name = "origin";

    let mut remote = repo.remote_with_fetch(
        remote_name,
        url,
        "+refs/*:refs/*",
    )?;

    let mut config = repo.config()?;
    config.set_bool(
        &format!("remote.{}.mirror", remote_name),
        true,
    )?;

    let refspecs: [&str; 0] = [];
    remote.fetch(&refspecs, None, None)?;

    if default_branch != "master" {
        repo_change_current_branch(&repo, default_branch)?;
    }

    Ok(())
}

/// Update remotes.
///
/// Works like:
///
/// ```shell
/// git remote update
/// ```
pub fn update<P: AsRef<Path>>(
    path: P,
) -> Result<(), Error> {
    let repo = git2::Repository::open_bare(path)?;

    for remote_opt in &repo.remotes()? {
        if let Some(remote_name) = remote_opt {
            let mut remote = repo.find_remote(remote_name)?;

            let mut fetch_options = git2::FetchOptions::new();
            fetch_options
                .prune(git2::FetchPrune::On)
                .download_tags(git2::AutotagOption::All);

            let refspecs: [&str; 0] = [];
            remote.fetch(&refspecs, Some(&mut fetch_options), None)?;
        }
    }

    Ok(())
}

/// Update the repository's description file.
pub fn update_description<P: AsRef<Path>>(
    repo_path: P,
    description: &str,
) -> Result<(), Error> {
    let description_path = repo_path.as_ref().join("description");

    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(description_path)?;

    if description.is_empty() {
        file.set_len(0)?;
    } else {
        writeln!(file, "{}", description)?;
    }

    Ok(())
}

/// Change the current branch of the repository at `repo_path` to
/// `default_branch`.
pub fn change_current_branch<P: AsRef<Path>>(
    repo_path: P,
    default_branch: &str,
) -> Result<(), Error> {
    let repo = git2::Repository::open_bare(repo_path)?;

    Ok(
        repo_change_current_branch(&repo, default_branch)?
    )
}

/// Change `repo`'s current branch to `default_branch`.
fn repo_change_current_branch(
    repo: &git2::Repository,
    default_branch: &str,
) -> Result<(), Error> {
    Ok(
        repo.set_head(
            &format!("refs/heads/{}", default_branch),
        )?
    )
}
