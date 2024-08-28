#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Internal(String),
    #[error("reqwest")]
    Reqwest(#[from] reqwest::Error),
}
