use std::time::Duration;

use futures_util::future::BoxFuture;
use http::Extensions;
use reqwest::{Request, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, Middleware, Next};
use reqwest_retry::{policies::ExponentialBackoff, RetryPolicy, RetryTransientMiddleware};

mod error;

pub use error::Error;

#[derive(Clone)]
pub struct WebClient(ClientWithMiddleware);

impl WebClient {
  const TIMEOUT: u64 = 75;

  pub fn new() -> Self {
    Self(Self::builder(Self::default_retry_policy()).build())
  }

  pub fn builder(retry_policy: impl RetryPolicy + Send + Sync + 'static) -> ClientBuilder {
    Self::from_reqwest_builder(Self::default_reqwest_builder(), retry_policy)
  }

  pub fn from_builder(builder: ClientBuilder) -> Self {
    Self(builder.build())
  }

  pub fn from_reqwest_builder(
    builder: reqwest::ClientBuilder,
    retry_policy: impl RetryPolicy + Send + Sync + 'static,
  ) -> ClientBuilder {
    ClientBuilder::new(builder.build().unwrap())
      .with(ExtensionCleaner)
      .with(RetryTransientMiddleware::new_with_policy(retry_policy))
      .with(increment_timeout)
  }

  pub fn default_reqwest_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
      .timeout(Duration::from_millis(Self::TIMEOUT))
      .user_agent(concat!("moss/", env!("CARGO_PKG_VERSION")))
      .use_rustls_tls()
      .tls_built_in_root_certs(false)
      .tls_built_in_native_certs(true)
  }

  pub fn default_retry_policy() -> ExponentialBackoff {
    ExponentialBackoff::builder()
      .retry_bounds(Duration::from_millis(20), Duration::from_millis(200))
      .build_with_max_retries(50)
  }

  pub async fn get(&self, url: String) -> Result<String, Error> {
    let request = self.0.get(url).build()?;

    Ok(
      self
        .0
        .execute(request)
        .await?
        .error_for_status()?
        .text()
        .await?,
    )
  }
}

#[derive(Debug, Clone, Copy)]
struct Timeout(u64);

fn increment_timeout<'a>(
  mut req: Request,
  extensions: &'a mut Extensions,
  next: Next<'a>,
) -> BoxFuture<'a, reqwest_middleware::Result<Response>> {
  let timeout = extensions.get_or_insert(Timeout(WebClient::TIMEOUT));

  req.timeout_mut().replace(Duration::from_millis(timeout.0));

  timeout.0 *= 2;

  next.run(req, extensions)
}

struct ExtensionCleaner;

#[async_trait::async_trait]
impl Middleware for ExtensionCleaner {
  async fn handle(
    &self,
    req: Request,
    extensions: &mut Extensions,
    next: Next<'_>,
  ) -> reqwest_middleware::Result<Response> {
    let mut extensions = extensions.clone();

    next.run(req, &mut extensions).await
  }
}
