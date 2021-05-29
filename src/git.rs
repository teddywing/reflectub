use std::path::Path;


pub fn mirror() -> Result<(), Box<dyn std::error::Error>> {
    // let builder = git2::build::RepoBuilder::new()
    //     .bare(true)
    //     .clone(
    //         "https://github.com/teddywing/google-calendar-rsvp.git",
    //         Path::new("/tmp/grsvp"),
    //     );

    let repo = git2::Repository::init_bare(Path::new("/tmp/grsvp"))?;

    let remote_name = "origin";

    let mut remote = repo.remote_with_fetch(
        remote_name,
        "https://github.com/teddywing/google-calendar-rsvp.git",
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
