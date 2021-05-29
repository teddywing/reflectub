use reflectub::{git, github};


fn main() {
    // let repos = github::fetch_repos().unwrap();
    //
    // dbg!(&repos);

    git::mirror().unwrap();
}
