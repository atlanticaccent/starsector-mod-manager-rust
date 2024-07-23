use std::time::Duration;

use futures::future::BoxFuture;
use http::Extensions;
use reqwest::{Request, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, Middleware, Next};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};

pub struct WebClient;

impl WebClient {
  pub(crate) const TIMEOUT: u64 = 75;

  pub fn new() -> ClientWithMiddleware {
    Self::builder(50).build()
  }

  pub fn builder(max_retries: u32) -> ClientBuilder {
    let retry_policy = ExponentialBackoff::builder()
      .retry_bounds(Duration::from_millis(20), Duration::from_millis(200))
      .build_with_max_retries(max_retries);
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
}

#[derive(Debug, Clone, Copy)]
struct Timeout(u64);

const MAX_TIMEOUT: u64 = 500;

fn increment_timeout<'a>(
  mut req: Request,
  extensions: &'a mut Extensions,
  next: Next<'a>,
) -> BoxFuture<'a, reqwest_middleware::Result<Response>> {
  let timeout = extensions.get_or_insert(Timeout(WebClient::TIMEOUT));

  req
    .timeout_mut()
    .replace(Duration::from_millis(timeout.0));

  timeout.0 *= 2;

  next.run(req, extensions)
}

pub struct ExtensionCleaner;

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
