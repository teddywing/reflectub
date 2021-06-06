use anyhow::{self, Context};
use chrono::DateTime;
use exitcode;
use filetime;
use futures::{executor, future};
use getopts::Options;
use parse_size::parse_size;
use sqlx;
use tokio;

use reflectub::{database, git, github};

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;


fn main() {
    run().unwrap();
}

fn print_usage(opts: &Options) {
    print!(
        "{}",
        opts.usage("usage: reflectub [options] -d DATABASE <github_username> <repository_path>"),
    );
}

fn run() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    let mut opts = Options::new();

    opts.optopt("d", "database", "SQLite database file path (required)", "DATABASE_FILE");
    opts.optopt("", "cgitrc", "base cgitrc file to copy to mirrored repositories", "CGITRC_FILE");
    opts.optopt("", "skip-larger-than", "skip repositories larger than SIZE", "SIZE");
    opts.optflag("h", "help", "print this help menu");
    opts.optflag("V", "version", "show the program version");

    let opt_matches = opts.parse(&args[1..])?;

    if opt_matches.opt_present("h") {
        print_usage(&opts);
        process::exit(exitcode::USAGE);
    }

    if opt_matches.opt_present("V") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        process::exit(exitcode::OK);
    }

    let database_file = opt_matches.opt_str("database")
        .ok_or(anyhow::anyhow!("missing required argument '--database'"))?;

    if opt_matches.free.len() != 2 {
        print_usage(&opts);
        process::exit(exitcode::USAGE);
    }

    let username = &opt_matches.free[0];
    let mirror_root = &opt_matches.free[1];

    let max_repo_size_bytes = opt_matches.opt_str("skip-larger-than")
        .map(|s|
            parse_size(&s)
                .with_context(|| format!(
                    "unable to parse max file size '{}'",
                    s
                ))
                .unwrap()
        );

    let base_cgitrc = opt_matches.opt_str("cgitrc")
        .map(|s| PathBuf::from(s));

    let rt = tokio::runtime::Builder::new_multi_thread().build()?;
    let _rt_guard = rt.enter();

    // let repos = github::fetch_repos(username).await?

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
            size: 81,
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
            size: 13,
            updated_at: "2020-11-11T22:49:53Z".to_owned(),
        },
    ];

    let db = Arc::new(
        tokio::sync::Mutex::new(
            executor::block_on(database::Db::connect(&database_file))?,
        )
    );

    executor::block_on(async {
        db.lock().await.create().await
    })?;

    let mut joins = Vec::new();

    for repo in test_repos {
        let db = db.clone();
        let mirror_root = mirror_root.clone();
        let base_cgitrc = base_cgitrc.clone();

        let join = tokio::spawn(async move {
            let mut db = db.lock().await;

            process_repo(
                &repo,
                &mut db,
                &mirror_root,
                base_cgitrc,
                max_repo_size_bytes,
            ).await
        });

        joins.push(join);
    }

    executor::block_on(future::join_all(joins));

    Ok(())
}

/// Mirror or update `repo`.
async fn process_repo(
    repo: &github::Repo,
    db: &mut database::Db,
    mirror_root: &str,
    base_cgitrc: Option<PathBuf>,
    max_repo_size_bytes: Option<u64>,
) -> anyhow::Result<()> {
    if let Some(max_repo_size_bytes) = max_repo_size_bytes {
        if is_repo_oversize(repo.size, max_repo_size_bytes) {
            return Ok(());
        }
    }

    let id = repo.id;
    let path = clone_path(&mirror_root, &repo);
    let db_repo = database::Repo::from(repo);

    match db.repo_get(id).await {
        // If we've already seen the repo and it's been updated, fetch the
        // latest.
        Ok(current_repo) => {
            if db.repo_is_updated(&db_repo).await? {
                update(&path, &current_repo, &repo)?;

                db.repo_update(&db_repo).await?;
            }
        },

        // If the repo doesn't exist, mirror it and store it in the
        // database.
        Err(database::Error::Db(sqlx::Error::RowNotFound)) => {
            mirror(
                &path,
                &repo,
                base_cgitrc.as_ref(),
            )?;

            db.repo_insert(db_repo).await?;
        },

        Err(e) => anyhow::bail!(e),
    }

    Ok(())
}


/// Return `true` if `size_kilobytes` is larger than `max_repo_size_bytes`.
fn is_repo_oversize(
    size_kilobytes: u64,
    max_repo_size_bytes: u64,
) -> bool {
    let size_bytes = size_kilobytes * 1000;

    if size_bytes > max_repo_size_bytes {
        return true;
    }

    false
}

/// Get the clone path for a repository.
///
/// If `repo` is a fork, add `/fork/` to `base_path`.
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

/// Mirror a repository.
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

/// Update a previously-mirrored repository.
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

    // Try updating times on the default ref.
    match filetime::set_file_times(
        &default_branch_ref,
        update_time,
        update_time,
    ) {
        Ok(_) => Ok(()),
        Err(e) => match e.kind() {
            // If the default ref file doesn't exist, update times on the
            // 'packed-refs' file.
            io::ErrorKind::NotFound => {
                let packed_refs_path = repo_path
                    .as_ref()
                    .join("packed-refs");

                Ok(
                    filetime::set_file_times(
                        &packed_refs_path,
                        update_time,
                        update_time,
                    )
                        .with_context(|| format!(
                            "unable to set mtime on '{}'",
                            &packed_refs_path.display(),
                        ))?
                )
            },
            _ => Err(e),
        },
    }
        .with_context(|| format!(
            "unable to set mtime on '{}'",
            &default_branch_ref.display(),
        ))?;

    Ok(())
}
