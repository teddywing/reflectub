use thiserror;

use std::path::Path;


#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("git error")]
    Git(#[from] git2::Error),
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
    description: Option<&str>,
) -> Result<(), Error> {
    let mut repo_init_options = git2::RepositoryInitOptions::new();
    repo_init_options.bare(true);

    if let Some(d) = description {
        repo_init_options.description(d);
    }

    let repo = git2::Repository::init_opts(path, &repo_init_options)?;

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
