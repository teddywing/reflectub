use reflectub::{git, github};

use std::path::Path;


fn main() {
    // let repos = github::fetch_repos().unwrap();
    //
    // dbg!(&repos);

    git::mirror(
        "https://github.com/teddywing/google-calendar-rsvp.git",
        Path::new("/tmp/grsvp"),
    ).unwrap();
}
