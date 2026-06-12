//! Shared HTTP transport used by all backends.
//!
//! A thin wrapper over a single [`reqwest::Client`] (rustls) that performs GET
//! requests, maps transport and status failures into [`DataError`], and decodes
//! JSON. Backends add their own auth headers via the `headers` argument.

use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::error::{DataError, Result};

/// Default request timeout.
const TIMEOUT: Duration = Duration::from_secs(15);

/// User-Agent sent with every request.
const USER_AGENT: &str = concat!("wc26/", env!("CARGO_PKG_VERSION"));

/// A cheap-to-clone HTTP client shared across backends.
#[derive(Debug, Clone)]
pub struct Http {
    client: reqwest::Client,
}

impl Http {
    /// Build a client with sensible defaults (timeout, User-Agent).
    ///
    /// # Errors
    /// Returns [`DataError::Transport`] if the underlying client cannot be built.
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|err| DataError::Transport(err.to_string()))?;
        Ok(Self { client })
    }

    /// Wrap an already-configured client (useful in tests).
    #[must_use]
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Fetch `url` and deserialize the JSON body into `T`.
    ///
    /// `headers` is a list of `(name, value)` pairs added to the request, used
    /// by backends for API-key auth.
    ///
    /// # Errors
    /// - [`DataError::Transport`] on connection/timeout failures.
    /// - [`DataError::RateLimited`] on HTTP 429.
    /// - [`DataError::Status`] on other non-success responses.
    /// - [`DataError::Decode`] if the body is not valid JSON for `T`.
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<T> {
        let bytes = self.get_bytes(url, headers).await?;
        serde_json::from_slice(&bytes).map_err(|err| DataError::Decode(err.to_string()))
    }

    /// Fetch `url` and return the raw response body as bytes, applying the same
    /// status-code handling as [`Self::get_json`].
    ///
    /// # Errors
    /// See [`Self::get_json`].
    pub async fn get_bytes(&self, url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>> {
        let mut req = self.client.get(url);
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        let resp = req
            .send()
            .await
            .map_err(|err| DataError::Transport(err.to_string()))?;

        let status = resp.status();
        if status.as_u16() == 429 {
            let retry_after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            return Err(DataError::RateLimited { retry_after });
        }
        if !status.is_success() {
            let message = resp
                .text()
                .await
                .ok()
                .map(|body| body.chars().take(200).collect::<String>())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| status.canonical_reason().unwrap_or("error").to_owned());
            return Err(DataError::Status {
                status: status.as_u16(),
                message,
            });
        }

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|err| DataError::Transport(err.to_string()))
    }
}
