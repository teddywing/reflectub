use reqwest::blocking::ClientBuilder;


const USER_AGENT: &'static str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
);


fn main() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Accept", "application/vnd.github.v3+json".parse().unwrap());

    let client = ClientBuilder::new()
        .user_agent(USER_AGENT)
        .default_headers(headers)
        .build()
        .unwrap();

    let response = client.request(
        reqwest::Method::GET,
        format!(
            "https://api.github.com/users/{}/repos",
            "teddywing",
        ),
    )
        .send()
        .unwrap();

    dbg!(&response);
}
