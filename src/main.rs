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
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rusqlite;

use reflectub::{database, git, github};

mod multi_error;
use multi_error::MultiError;

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;


fn main() {
    match run() {
        Ok(_) => (),
        Err(e) => {
            e
                .into_iter()
                .for_each(|e| eprintln!("error: {:#}", e));

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

fn run() -> Result<(), MultiError> {
    let args: Vec<String> = env::args().collect();

    let mut opts = Options::new();

    opts.optopt("d", "database", "SQLite database file path (required)", "DATABASE_FILE");
    opts.optopt("", "cgitrc", "base cgitrc file to copy to mirrored repositories", "CGITRC_FILE");
    opts.optopt("", "skip-larger-than", "skip repositories larger than SIZE", "SIZE");
    opts.optflag("h", "help", "print this help menu");
    opts.optflag("V", "version", "show the program version");

    let opt_matches = opts.parse(&args[1..])
        .map_err(anyhow::Error::new)?;

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

    let repos = github::fetch_repos(username)
        .context("unable to fetch GitHub repositories")?;

    let db = database::Db::connect(&database_file)
        .context("unable to connect to database")?;

    db.create()
        .context("unable to create database")?;

    let errors: Vec<_> = repos
        .par_iter()
        .map(|repo| {
            (
                &repo.name,
                process_repo(
                    &repo,
                    &db,
                    &mirror_root,
                    base_cgitrc.as_ref(),
                    max_repo_size_bytes,
                ),
            )
        })
        .filter(|(_, r)| r.is_err())

        // `error` should always be an error.
        .map(|(name, error)| {
            error
                .err()
                .unwrap()
                .context(name.clone())
        })
        .collect();

    if errors.len() > 0 {
        return Err(MultiError::from(errors))
    }

    Ok(())
}

/// Mirror or update `repo`.
fn process_repo<P: AsRef<Path>>(
    repo: &github::Repo,
    db: &database::Db,
    mirror_root: &str,
    base_cgitrc: Option<P>,
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
                path,
                &repo,
                base_cgitrc,
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
fn mirror<P1, P2>(
    clone_path: P1,
    repo: &github::Repo,
    base_cgitrc: Option<P2>,
) -> anyhow::Result<()>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    git::mirror(
        &repo.git_url,
        &clone_path,
        repo.description(),
        &repo.default_branch,
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

    if repo.default_branch != "master" {
        repo_cgitrc_set_defbranch(&clone_path, &repo.default_branch)?;
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

    if let Some(default_branch) = &current_repo.default_branch {
        if default_branch != &updated_repo.default_branch {
            git::change_current_branch(
                &repo_path,
                &updated_repo.default_branch,
            )?;

            repo_cgitrc_set_defbranch(&repo_path, &updated_repo.default_branch)?;
        }
    }

    update_mtime(&repo_path, &updated_repo)?;

    Ok(())
}

/// Set the mtime of the repository to GitHub's `pushed_at` time.
///
/// Used for CGit "age" sorting.
fn update_mtime<P: AsRef<Path>>(
    repo_path: P,
    repo: &github::Repo,
) -> anyhow::Result<()> {
    let update_time = filetime::FileTime::from_system_time(
        DateTime::parse_from_rfc3339(&repo.pushed_at)
            .with_context(|| format!(
                "unable to parse update time from '{}'",
                &repo.pushed_at,
            ))?
            .into()
    );

    let default_branch_ref = repo_path
        .as_ref()
        .join("refs/heads")
        .join(&repo.default_branch);

    // Try updating times on the default ref.
    match filetime::set_file_times(
        &default_branch_ref,
        update_time,
        update_time,
    ) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // If the default ref file doesn't exist, update times on the
            // 'packed-refs' file.
            let packed_refs_path = repo_path
                .as_ref()
                .join("packed-refs");

            match filetime::set_file_times(
                &packed_refs_path,
                update_time,
                update_time,
            ) {
                Ok(_) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    // In the absence of a 'packed-refs' file, create a CGit
                    // agefile and add the update time to it.
                    Ok(set_agefile_time(&repo_path, &repo.pushed_at)?)
                },
                Err(e) => Err(e),
            }
                .with_context(|| format!(
                    "unable to set mtime on '{}'",
                    &packed_refs_path.display(),
                ))?;

            Ok(())
        },
        Err(e) => Err(e),
    }
        .with_context(|| format!(
            "unable to set mtime on '{}'",
            &default_branch_ref.display(),
        ))?;

    Ok(())
}

/// Write `update_time` into the repo's `info/web/last-modified` file.
fn set_agefile_time<P: AsRef<Path>>(
    repo_path: P,
    update_time: &str,
) -> anyhow::Result<()> {
    let agefile_dir = repo_path.as_ref().join("info/web");
    fs::DirBuilder::new()
        .create(&agefile_dir)
        .with_context(|| format!(
            "unable to create directory '{}'",
            &agefile_dir.display(),
        ))?;

    let agefile_path = agefile_dir.join("last-modified");
    let mut agefile = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(&agefile_path)
        .with_context(|| format!(
            "unable to open '{}'",
            &agefile_path.display(),
        ))?;

    writeln!(agefile, "{}", &update_time)
        .with_context(|| format!(
            "unable to write to '{}'",
            &agefile_path.display(),
        ))?;

    Ok(())
}

/// Set the default CGit branch in the repository's "cgitrc" file.
fn repo_cgitrc_set_defbranch<P: AsRef<Path>>(
    repo_path: P,
    default_branch: &str,
) -> anyhow::Result<()> {
    repo_cgitrc_append(
        &repo_path,
        &format!("defbranch={}", default_branch),
    )?;

    Ok(())
}

/// Append `config` to the repo-local "cgitrc" file.
fn repo_cgitrc_append<P: AsRef<Path>>(
    repo_path: P,
    config: &str,
) -> anyhow::Result<()> {
    let cgitrc_path = repo_path
        .as_ref()
        .join("cgitrc");

    let mut cgitrc_file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&cgitrc_path)
        .with_context(|| format!(
            "unable to open '{}'",
            &cgitrc_path.display(),
        ))?;

    writeln!(cgitrc_file, "{}", config)
        .with_context(|| format!(
            "unable to write to '{}'",
            &cgitrc_path.display(),
        ))?;

    Ok(())
}
