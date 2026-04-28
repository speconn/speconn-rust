use crate::envelope::{decode_envelope, FLAG_END_STREAM};
use crate::error::{Code, SpeconnError};
use crate::transport::Transport;

/// A request builder that carries message + headers.
pub struct RequestBuilder<'a, T: Transport, Req: serde::Serialize> {
    client: &'a SpeconnClient<T>,
    path: &'a str,
    req: Req,
    headers: Vec<(String, String)>,
}

impl<'a, T: Transport, Req: serde::Serialize> RequestBuilder<'a, T, Req> {
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    pub async fn call<Res: serde::de::DeserializeOwned>(self) -> Result<Res, SpeconnError> {
        let url = format!("{}{}", self.client.base_url, self.path);
        let body = serde_json::to_vec(&self.req)
            .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?;
        let headers: Vec<(&str, &str)> = self.headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let resp = self.client.transport.post(&url, "application/json", &body, &headers).await?;
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
        let body = serde_json::to_vec(&self.req)
            .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?;
        let mut headers: Vec<(&str, &str)> = vec![("connect-protocol-version", "1")];
        let owned: Vec<(&str, &str)> = self.headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        headers.extend(owned.iter().copied());
        let resp = self.client.transport.post(&url, "application/connect+json", &body, &headers).await?;
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

pub struct SpeconnClient<T: Transport> {
    base_url: String,
    transport: T,
}

impl<T: Transport> SpeconnClient<T> {
    pub fn new(base_url: &str, transport: T) -> Self {
        SpeconnClient {
            base_url: base_url.trim_end_matches('/').to_string(),
            transport,
        }
    }

    pub fn request<Req: serde::Serialize>(&self, path: &str, req: Req) -> RequestBuilder<'_, T, Req> {
        RequestBuilder {
            client: self,
            path,
            req,
            headers: Vec::new(),
        }
    }
}

#[cfg(feature = "reqwest")]
impl SpeconnClient<crate::transport::ReqwestTransport> {
    pub fn new_reqwest(base_url: &str) -> Self {
        Self::new(base_url, crate::transport::ReqwestTransport::new())
    }
}

#[cfg(feature = "isahc")]
impl SpeconnClient<crate::transport::IsahcTransport> {
    pub fn new_isahc(base_url: &str) -> Self {
        Self::new(base_url, crate::transport::IsahcTransport::new())
    }
}
