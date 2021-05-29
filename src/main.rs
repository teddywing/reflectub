use reqwest::blocking::Client;


fn main() {
    let client = Client::new();

    let response = client.request(
        reqwest::Method::GET,
        format!(
            "https://api.github.com/users/{}/repos",
            "teddywing",
        ),
    )
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "reflectub")
        .send()
        .unwrap();

    dbg!(&response);
}
