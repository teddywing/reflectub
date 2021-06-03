use anyhow::{self, Context};
use chrono::DateTime;
use filetime;
use sqlx;
use tokio;

use reflectub::{database, git, github};

use std::fs;
use std::path::{Path, PathBuf};


#[tokio::main]
async fn main() {
    // let repos = github::fetch_repos("teddywing").await.unwrap();
    //
    // dbg!(&repos);

    // git::mirror(
    //     "https://github.com/teddywing/google-calendar-rsvp.git",
    //     Path::new("/tmp/grsvp"),
    // ).unwrap();

    // git::update(
    //     Path::new("/tmp/grsvp"),
    // ).unwrap();

    run().await.unwrap();
}

async fn run() -> anyhow::Result<()> {
    let test_repos = vec![
        github::Repo {
            id: 345367151,
            name: "DDHotKey".to_owned(),
            description: Some(
                "Simple Cocoa global hotkeys".to_owned(),
            ),
            fork: true,
            git_url: "git://github.com/teddywing/DDHotKey.git".to_owned(),
            default_branch: "master".to_owned(),
            updated_at: "2021-03-07T14:27:06Z".to_owned(),
        },
        github::Repo {
            id: 312106271,
            name: "apple-developer-objc".to_owned(),
            description: Some(
                "A user script that forces Apple Developer documentation to use Objective-C".to_owned(),
            ),
            fork: false,
            git_url: "git://github.com/teddywing/apple-developer-objc.git".to_owned(),
            default_branch: "master".to_owned(),
            updated_at: "2020-11-11T22:49:53Z".to_owned(),
        },
    ];

    let mut db = database::Db::connect("test.db").await.unwrap();

    db.create().await.unwrap();

    // If repo !exists
    //   insert
    //   mirror
    // Else
    //   Update updated_at
    //   fetch

    for repo in test_repos {
        let id = repo.id;
        let path = clone_path("/tmp", &repo);
        let db_repo = database::Repo::from(&repo);

        match db.repo_get(id).await {
            Ok(current_repo) => {
                if db.repo_is_updated(&db_repo).await.unwrap() {
                    update(&path, &current_repo, &repo).unwrap();

                    db.repo_update(&db_repo).await.unwrap();
                }
            },

            Err(database::Error::Db(sqlx::Error::RowNotFound)) => {
                mirror(
                    &path,
                    &repo,
                    Some(&"./cgitrc".to_owned().into()),
                ).unwrap();

                db.repo_insert(db_repo).await.unwrap();
            },

            e => panic!("{:?}", e),
        }
    }

    Ok(())
}


fn clone_path<P: AsRef<Path>>(base_path: P, repo: &github::Repo) -> PathBuf {
    let git_dir = format!("{}.git", repo.name);

    if repo.fork {
        base_path
            .as_ref()
            .join("fork")
            .join(git_dir)
    } else {
        base_path
            .as_ref()
            .join(git_dir)
    }
}

fn mirror<P: AsRef<Path>>(
    clone_path: P,
    repo: &github::Repo,
    base_cgitrc: Option<P>,
) -> anyhow::Result<()> {
    git::mirror(
        &repo.git_url,
        &clone_path,
        repo.description(),
    )?;

    // Copy the base cgitrc file into the newly-cloned repository.
    if let Some(base_cgitrc) = base_cgitrc {
        let cgitrc_path = clone_path.as_ref().join("cgitrc");

        fs::copy(&base_cgitrc, &cgitrc_path)
            .with_context(|| format!(
                "unable to copy '{}' to '{}'",
                "./cgitrc",
                &cgitrc_path.display(),
            ))?;
    }

    update_mtime(&clone_path, &repo)?;

    Ok(())
}

fn update<P: AsRef<Path>>(
    repo_path: P,
    current_repo: &database::Repo,
    updated_repo: &github::Repo,
) -> anyhow::Result<()> {
    git::update(&repo_path)?;

    let remote_description = updated_repo.description();

    if current_repo.description() != remote_description {
        git::update_description(&repo_path, remote_description)?;
    }

    update_mtime(&repo_path, &updated_repo)?;

    Ok(())
}

/// Set the mtime of the repository to GitHub's `updated_at` time.
///
/// Used for CGit "age" sorting.
fn update_mtime<P: AsRef<Path>>(
    repo_path: P,
    repo: &github::Repo,
) -> anyhow::Result<()> {
    let default_branch_ref = repo_path
        .as_ref()
        .join("refs/heads")
        .join(&repo.default_branch);

    let update_time = filetime::FileTime::from_system_time(
        DateTime::parse_from_rfc3339(&repo.updated_at)?.into()
    );

    filetime::set_file_times(&default_branch_ref, update_time, update_time)
        .with_context(|| format!(
            "unable to set mtime on '{}'",
            &default_branch_ref.display(),
        ))?;

    Ok(())
}
