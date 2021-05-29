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
