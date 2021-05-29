use serde::Deserialize;


#[derive(Debug, Deserialize)]
pub struct Repo {
    id: usize,
    name: String,
    description: Option<String>,
    fork: bool,
    git_url: String,
    default_branch: String,
    updated_at: String,  // TODO: Maybe parse to date?
}
