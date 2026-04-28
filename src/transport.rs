use std::future::Future;
use std::pin::Pin;

use crate::error::{Code, SpeconnError};

pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// HttpClient is the trait Speconn expects HTTP clients to implement.
/// reqwest::Client implements HttpClient via the provided blanket impl.
pub trait HttpClient: Send + Sync {
    fn post(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, SpeconnError>> + Send>>;
}

#[cfg(feature = "reqwest")]
impl HttpClient for reqwest::Client {
    fn post(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, SpeconnError>> + Send>> {
        let client = self.clone();
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
            let resp_body = resp.bytes().await
                .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?.to_vec();
            Ok(HttpResponse { status, body: resp_body })
        })
    }
}

pub fn default_http_client() -> reqwest::Client {
    reqwest::Client::new()
}
