use std::future::Future;
use std::pin::Pin;

use crate::error::{Code, SpeconnError};

pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub trait SpeconnTransport: Send + Sync {
    fn post(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, SpeconnError>> + Send>>;
}

#[cfg(feature = "reqwest")]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

#[cfg(feature = "reqwest")]
impl ReqwestTransport {
    pub fn new() -> Self {
        Self { client: reqwest::Client::new() }
    }
}

#[cfg(feature = "reqwest")]
impl Default for ReqwestTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "reqwest")]
impl SpeconnTransport for ReqwestTransport {
    fn post(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, SpeconnError>> + Send>> {
        let client = self.client.clone();
        let url = url.to_string();
        let headers: Vec<(String, String)> = headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();

        Box::pin(async move {
            let mut req = client.post(&url).body(body);
            for (k, v) in &headers {
                req = req.header(k.as_str(), v.as_str());
            }
            let resp = req.send().await
                .map_err(|e| SpeconnError::new(Code::Unavailable, e.to_string()))?;
            let status = resp.status().as_u16();
            let resp_headers: Vec<(String, String)> = resp.headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            let resp_body = resp.bytes().await
                .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?.to_vec();
            Ok(HttpResponse { status, headers: resp_headers, body: resp_body })
        })
    }
}
