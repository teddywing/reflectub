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
    #[error("mirror: cannot create repo '{path}'")]
    MirrorCreateRepo {
        source: git2::Error,
        path: String,
    },
    #[error("mirror: cannot add remote '{remote_name}:{url}'")]
    MirrorAddRemote {
        source: git2::Error,
        remote_name: String,
        url: String,
    },
    #[error("mirror: cannot get repo config")]
    MirrorConfigGet(#[source] git2::Error),
    #[error("mirror: cannot set 'mirror' flag on remote '{remote_name}'")]
    MirrorRemoteEnableMirror {
        source: git2::Error,
        remote_name: String,
    },
    #[error("mirror: cannot fetch from remote '{remote_name}'")]
    MirrorFetch {
        source: git2::Error,
        remote_name: String,
    },

    #[error("update: cannot open repo '{path}'")]
    UpdateOpenRepo {
        source: git2::Error,
        path: String,
    },
    #[error("update: cannot get remotes for '{path}'")]
    UpdateGetRemotes {
        source: git2::Error,
        path: String,
    },
    #[error("update: cannot find remote '{remote_name}'")]
    UpdateFindRemote {
        source: git2::Error,
        remote_name: String,
    },
    #[error("update: cannot fetch from remote '{remote_name}")]
    UpdateFetch {
        source: git2::Error,
        remote_name: String,
    },

    #[error("{action}: cannot switch to branch '{branch}'")]
    GitChangeBranch {
        source: git2::Error,
        action: String,
        branch: String,
    },

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
pub fn mirror<P: AsRef<Path> + Copy>(
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
        .map_err(|e| Error::MirrorCreateRepo {
            source: e,
            path: format!("{}", path.as_ref().display()),
        })?;

    let remote_name = "origin";

    let mut remote = repo.remote_with_fetch(
        remote_name,
        url,
        "+refs/*:refs/*",
    )
        .map_err(|e| Error::MirrorAddRemote {
            source: e,
            remote_name: remote_name.to_owned(),
            url: url.to_owned(),
        })?;

    let mut config = repo.config()
        .map_err(|e| Error::MirrorConfigGet(e))?;
    config.set_bool(
        &format!("remote.{}.mirror", remote_name),
        true,
    )
        .map_err(|e| Error::MirrorRemoteEnableMirror {
            source: e,
            remote_name: remote_name.to_owned(),
        })?;

    let refspecs: [&str; 0] = [];
    remote.fetch(&refspecs, None, None)
        .map_err(|e| Error::MirrorFetch {
            source: e,
            remote_name: remote_name.to_owned(),
        })?;

    if default_branch != "master" {
        repo_change_current_branch(&repo, default_branch)
            .map_err(|e| Error::GitChangeBranch {
                source: e,
                action: "mirror".to_owned(),
                branch: default_branch.to_owned(),
            })?;
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
pub fn update<P: AsRef<Path> + Copy>(
    path: P,
) -> Result<(), Error> {
    let repo = git2::Repository::open_bare(path)
        .map_err(|e| Error::UpdateOpenRepo {
            source: e,
            path: format!("{}", path.as_ref().display()),
        })?;

    let remotes = &repo.remotes()
        .map_err(|e| Error::UpdateGetRemotes {
            source: e,
            path: format!("{}", path.as_ref().display()),
        })?;
    for remote_opt in remotes {
        if let Some(remote_name) = remote_opt {
            let mut remote = repo.find_remote(remote_name)
                .map_err(|e| Error::UpdateFindRemote {
                    source: e,
                    remote_name: remote_name.to_owned(),
                })?;

            let mut fetch_options = git2::FetchOptions::new();
            fetch_options
                .prune(git2::FetchPrune::On)
                .download_tags(git2::AutotagOption::All);

            let refspecs: [&str; 0] = [];
            remote.fetch(&refspecs, Some(&mut fetch_options), None)
                .map_err(|e| Error::UpdateFetch {
                    source: e,
                    remote_name: remote_name.to_owned(),
                })?;
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
) -> Result<(), git2::Error> {
    Ok(
        repo.set_head(
            &format!("refs/heads/{}", default_branch),
        )?
    )
}
