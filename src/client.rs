use crate::envelope::{decode_envelope, FLAG_END_STREAM};
use crate::error::{Code, SpeconnError};
use crate::transport::HttpClient;

/// A request builder that carries message + headers.
pub struct RequestBuilder<'a, C: HttpClient> {
    client: &'a SpeconnClient<C>,
    path: &'a str,
    req_body: Vec<u8>,
    headers: Vec<(String, String)>,
}

impl<'a, C: HttpClient> RequestBuilder<'a, C> {
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    pub async fn call<Res: serde::de::DeserializeOwned>(self) -> Result<Res, SpeconnError> {
        let url = format!("{}{}", self.client.base_url, self.path);
        let mut headers: Vec<(&str, &str)> = vec![("content-type", "application/json")];
        let owned: Vec<(&str, &str)> = self.headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        headers.extend(owned.iter().copied());
        let resp = self.client.http_client.post(&url, &headers, self.req_body).await?;
        if resp.status >= 400 {
            let err: serde_json::Value = serde_json::from_slice(&resp.body).unwrap_or(serde_json::json!({}));
            return Err(SpeconnError::new(
                Code::from_str(err["code"].as_str().unwrap_or("unknown")),
                err["message"].as_str().unwrap_or("").to_string(),
            ));
        }
        serde_json::from_slice(&resp.body).map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))
    }

    pub async fn stream<Res: serde::de::DeserializeOwned>(self) -> Result<Vec<Res>, SpeconnError> {
        let url = format!("{}{}", self.client.base_url, self.path);
        let mut headers: Vec<(&str, &str)> = vec![
            ("content-type", "application/connect+json"),
            ("connect-protocol-version", "1"),
        ];
        let owned: Vec<(&str, &str)> = self.headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        headers.extend(owned.iter().copied());
        let resp = self.client.http_client.post(&url, &headers, self.req_body).await?;
        if resp.status >= 400 {
            let err: serde_json::Value = serde_json::from_slice(&resp.body).unwrap_or(serde_json::json!({}));
            return Err(SpeconnError::new(
                Code::from_str(err["code"].as_str().unwrap_or("unknown")),
                err["message"].as_str().unwrap_or("").to_string(),
            ));
        }
        let mut results = Vec::new();
        let mut pos = 0;
        while pos < resp.body.len() {
            if resp.body.len() - pos < 5 { break; }
            let (flags, payload) = decode_envelope(&resp.body[pos..])
                .map_err(|e| SpeconnError::new(Code::Internal, e))?;
            pos += 5 + payload.len();
            if flags & FLAG_END_STREAM != 0 {
                let trailer: serde_json::Value = serde_json::from_slice(payload).unwrap_or(serde_json::json!({}));
                if let Some(err) = trailer.get("error") {
                    return Err(SpeconnError::new(
                        Code::from_str(err["code"].as_str().unwrap_or("unknown")),
                        err["message"].as_str().unwrap_or("").to_string(),
                    ));
                }
                break;
            }
            let msg: Res = serde_json::from_slice(payload)
                .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?;
            results.push(msg);
        }
        Ok(results)
    }
}

pub struct SpeconnClient<C: HttpClient> {
    base_url: String,
    http_client: C,
}

impl<C: HttpClient> SpeconnClient<C> {
    pub fn new(base_url: &str, http_client: C) -> Self {
        SpeconnClient {
            base_url: base_url.trim_end_matches('/').to_string(),
            http_client,
        }
    }

    pub fn request<'a, Req: serde::Serialize>(&'a self, path: &'a str, req: Req) -> RequestBuilder<'a, C> {
        let req_body = serde_json::to_vec(&req).unwrap_or_default();
        RequestBuilder {
            client: self,
            path,
            req_body,
            headers: Vec::new(),
        }
    }
}

#[cfg(feature = "reqwest")]
impl SpeconnClient<reqwest::Client> {
    pub fn new_default(base_url: &str) -> Self {
        Self::new(base_url, reqwest::Client::new())
    }
}
