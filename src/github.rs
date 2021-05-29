use reqwest::blocking::ClientBuilder;

use crate::repo::Repo;


const USER_AGENT: &'static str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
);


pub fn fetch_repos() -> Result<Vec<Repo>, Box<dyn std::error::Error>> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Accept", "application/vnd.github.v3+json".parse().unwrap());

    let client = ClientBuilder::new()
        .user_agent(USER_AGENT)
        .default_headers(headers)
        .build()
        .unwrap();

    let repos = client.request(
        reqwest::Method::GET,
        format!(
            "https://api.github.com/users/{}/repos",
            "teddywing",
        ),
    )
        .send()
        .unwrap()
        .json::<Vec<Repo>>()
        .unwrap();

    Ok(repos)
}
