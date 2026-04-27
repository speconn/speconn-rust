use std::future::Future;
use std::pin::Pin;

use crate::error::{Code, SpeconnError};

pub struct TransportResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

pub trait Transport: Send + Sync {
    fn post(
        &self,
        url: &str,
        content_type: &str,
        body: &[u8],
        headers: &[(&str, &str)],
    ) -> Pin<Box<dyn Future<Output = Result<TransportResponse, SpeconnError>> + Send>>;
}

#[cfg(feature = "reqwest")]
mod reqwest_impl {
    use super::*;

    pub struct ReqwestTransport {
        client: reqwest::Client,
    }

    impl ReqwestTransport {
        pub fn new() -> Self {
            Self { client: reqwest::Client::new() }
        }
    }

    impl Default for ReqwestTransport {
        fn default() -> Self { Self::new() }
    }

    impl Transport for ReqwestTransport {
        fn post(
            &self,
            url: &str,
            content_type: &str,
            body: &[u8],
            headers: &[(&str, &str)],
        ) -> Pin<Box<dyn Future<Output = Result<TransportResponse, SpeconnError>> + Send>> {
            let client = self.client.clone();
            let url = url.to_string();
            let content_type = content_type.to_string();
            let body = body.to_vec();
            let headers: Vec<(String, String)> = headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();

            Box::pin(async move {
                let mut req = client.post(&url).header("content-type", &content_type).body(body);
                for (k, v) in &headers {
                    req = req.header(k.as_str(), v.as_str());
                }
                let resp = req.send().await.map_err(|e| SpeconnError::new(Code::Unavailable, e.to_string()))?;
                let status = resp.status().as_u16();
                let resp_body = resp.bytes().await.map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?.to_vec();
                Ok(TransportResponse { status, body: resp_body })
            })
        }
    }
}

#[cfg(feature = "reqwest")]
pub use reqwest_impl::ReqwestTransport;

#[cfg(feature = "isahc")]
mod isahc_impl {
    use super::*;

    pub struct IsahcTransport {
        client: isahc::HttpClient,
    }

    impl IsahcTransport {
        pub fn new() -> Self {
            Self { client: isahc::HttpClient::new().unwrap() }
        }
    }

    impl Default for IsahcTransport {
        fn default() -> Self { Self::new() }
    }

    impl Transport for IsahcTransport {
        fn post(
            &self,
            url: &str,
            content_type: &str,
            body: &[u8],
            headers: &[(&str, &str)],
        ) -> Pin<Box<dyn Future<Output = Result<TransportResponse, SpeconnError>> + Send>> {
            let client = self.client.clone();
            let url = url.to_string();
            let content_type = content_type.to_string();
            let body = body.to_vec();
            let headers: Vec<(String, String)> = headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();

            Box::pin(async move {
                use isahc::http::Request;
                use isahc::ReadResponseExt;

                let mut builder = Request::post(&url)
                    .header("content-type", &content_type)
                    .body(body)
                    .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?;

                for (k, v) in &headers {
                    builder.headers_mut().unwrap().insert(
                        isahc::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                        v.parse().unwrap(),
                    );
                }

                let mut resp = client.send_async(builder).await.map_err(|e| SpeconnError::new(Code::Unavailable, e.to_string()))?;
                let status = resp.status().as_u16();
                let mut resp_body = Vec::new();
                resp.copy_to(&mut resp_body).await.map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?;
                Ok(TransportResponse { status, body: resp_body })
            })
        }
    }
}

#[cfg(feature = "isahc")]
pub use isahc_impl::IsahcTransport;
