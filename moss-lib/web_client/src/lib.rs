use std::time::Duration;

use futures::future::BoxFuture;
use http::Extensions;
use reqwest::{Request, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, Middleware, Next};
use reqwest_retry::{policies::ExponentialBackoff, RetryPolicy, RetryTransientMiddleware};

mod error;

pub use error::Error;

pub struct WebClient(ClientWithMiddleware);

impl WebClient {
  const TIMEOUT: u64 = 75;

  pub fn new() -> Self {
    Self(
      Self::builder(
        ExponentialBackoff::builder()
          .retry_bounds(Duration::from_millis(20), Duration::from_millis(200))
          .build_with_max_retries(50),
      )
      .build(),
    )
  }

  pub fn builder(retry_policy: impl RetryPolicy + Send + Sync + 'static) -> ClientBuilder {
    ClientBuilder::new(
      reqwest::Client::builder()
        .brotli(true)
        .gzip(true)
        .deflate(true)
        .timeout(Duration::from_millis(Self::TIMEOUT))
        .user_agent("StarsectorModManager")
        .build()
        .unwrap(),
    )
    .with(ExtensionCleaner)
    .with(RetryTransientMiddleware::new_with_policy(retry_policy))
    .with(increment_timeout)
  }

  pub fn from_builder(builder: ClientBuilder) -> Self {
    Self(builder.build())
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
