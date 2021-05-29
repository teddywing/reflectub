use reflectub::github;


fn main() {
    let repos = github::fetch_repos().unwrap();

    dbg!(&repos);
}
