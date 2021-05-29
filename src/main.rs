use tokio;

use reflectub::{database, git, github};

use std::path::Path;


#[tokio::main]
async fn main() {
    // let repos = github::fetch_repos().unwrap();
    //
    // dbg!(&repos);

    // git::mirror(
    //     "https://github.com/teddywing/google-calendar-rsvp.git",
    //     Path::new("/tmp/grsvp"),
    // ).unwrap();

    // git::update(
    //     Path::new("/tmp/grsvp"),
    // ).unwrap();

    let db = database::Db::connect("sqlite::memory:").await.unwrap();
}
