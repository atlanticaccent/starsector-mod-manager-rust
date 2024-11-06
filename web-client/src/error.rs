#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("Reqwest error")]
  ReqwestError(#[from] reqwest::Error),
  #[error("Middleware error")]
  MiddlewareError(#[from] reqwest_middleware::Error),
}
