use std::path::Path;


pub fn mirror<P: AsRef<Path>>(
    url: &str,
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = git2::Repository::init_bare(path)?;

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

pub fn update<P: AsRef<Path>>(
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
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
