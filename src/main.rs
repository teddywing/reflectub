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


use anyhow::{self, Context};
use chrono::DateTime;
use exitcode;
use filetime;
use getopts::Options;
use parse_size::parse_size;
use r2d2_sqlite::SqliteConnectionManager;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rusqlite;

use reflectub::{database, git, github};

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Arc, Mutex};


fn main() {
    match run() {
        Ok(_) => (),
        Err(e) => {
            eprint!("error");

            for cause in e.chain() {
                eprint!(": {}", cause);
            }

            eprintln!();

            process::exit(exitcode::SOFTWARE);
        },
    };
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

    // Parse the maximum repo size and return an error if it fails. If the size
    // is `None`, set the value to `None`.
    let max_repo_size_bytes = opt_matches.opt_str("skip-larger-than")
        .map_or(
            Ok(None),
            |s|
                parse_size(&s)
                    .map(|s| Some(s))
                    .with_context(|| format!(
                        "unable to parse max file size '{}'",
                        s
                    ))
        )?;

    let base_cgitrc = opt_matches.opt_str("cgitrc")
        .map(|s| PathBuf::from(s));

    let repos = github::fetch_repos(username)?;

    let db = database::Db::connect(&database_file)
        .context("unable to connect to database")?;

    db.create()
        .context("unable to create database")?;

    let _results: anyhow::Result<()> = repos[..2].par_iter()
        .map(|repo| {
            dbg!("Thread", std::thread::current().id());

            process_repo(
                &repo,
                &db,
                &mirror_root,
                base_cgitrc.clone(), // TODO: Can we avoid cloning
                max_repo_size_bytes,
            )
        })
        .collect();
    // TODO: Return errors

    Ok(())
}

/// Mirror or update `repo`.
fn process_repo(
    repo: &github::Repo,
    db: &database::Db,
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

    match db.repo_get(id) {
        // If we've already seen the repo and it's been updated, fetch the
        // latest.
        Ok(current_repo) => {
            if db.repo_is_updated(&db_repo)? {
                update(&path, &current_repo, &repo)?;

                db.repo_update(&db_repo)?;
            }
        },

        // If the repo doesn't exist, mirror it and store it in the
        // database.
        Err(database::Error::Db(rusqlite::Error::QueryReturnedNoRows)) => {
            mirror(
                &path,
                &repo,
                base_cgitrc.as_ref(),
            )?;

            db.repo_insert(db_repo)?;
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
