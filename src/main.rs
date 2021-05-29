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

    let test_repos = vec![
        github::Repo {
            id: 18086664,
            name: "angular.js".to_owned(),
            description: None,
            fork: true,
            git_url: "git://github.com/teddywing/angular.js.git".to_owned(),
            default_branch: "master".to_owned(),
            updated_at: "2014-03-25T06:55:16Z".to_owned(),
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

    // db.create().await.unwrap();

    // If repo !exists
    //   insert
    //   mirror
    // Else
    //   Update updated_at
    //   fetch

    for repo in test_repos {
        let r = db.repo_get(repo.id).await.unwrap();

        dbg!(r);
    }
}
