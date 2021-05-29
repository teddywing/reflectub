use reqwest::blocking::ClientBuilder;
use serde::Deserialize;


const USER_AGENT: &'static str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
);


#[derive(Debug, Deserialize)]
pub struct Repo {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub fork: bool,
    pub git_url: String,
    pub default_branch: String,
    pub updated_at: String,  // TODO: Maybe parse to date?
}


pub fn fetch_repos() -> Result<Vec<Repo>, Box<dyn std::error::Error>> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Accept", "application/vnd.github.v3+json".parse()?);

    let client = ClientBuilder::new()
        .user_agent(USER_AGENT)
        .default_headers(headers)
        .build()?;

    let repos = client.request(
        reqwest::Method::GET,
        format!(
            "https://api.github.com/users/{}/repos",
            "teddywing",
        ),
    )
        .send()?
        .json::<Vec<Repo>>()?;

    Ok(repos)
}
