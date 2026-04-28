use crate::envelope::{decode_envelope, FLAG_END_STREAM};
use crate::error::{Code, SpeconnError};
use crate::transport::Transport;

pub struct CallOption {
    headers: Vec<(String, String)>,
}

impl CallOption {
    pub fn headers(&self) -> Vec<(&str, &str)> {
        self.headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
    }
}

pub fn with_header(key: &str, value: &str) -> CallOption {
    CallOption {
        headers: vec![(key.to_string(), value.to_string())],
    }
}

pub fn with_headers(headers: Vec<(&str, &str)>) -> CallOption {
    CallOption {
        headers: headers.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
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

    pub async fn call<Req: serde::Serialize, Res: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        req: &Req,
        options: &[CallOption],
    ) -> Result<Res, SpeconnError> {
        let url = format!("{}{}", self.base_url, path);
        let body = serde_json::to_vec(req).map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?;

        let headers: Vec<(&str, &str)> = options.iter().flat_map(|o| o.headers()).collect();
        let resp = self.transport.post(&url, "application/json", &body, &headers).await?;

        if resp.status >= 400 {
            let err: serde_json::Value = serde_json::from_slice(&resp.body).unwrap_or(serde_json::json!({}));
            return Err(SpeconnError::new(
                Code::from_str(err["code"].as_str().unwrap_or("unknown")),
                err["message"].as_str().unwrap_or("").to_string(),
            ));
        }

        serde_json::from_slice(&resp.body).map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))
    }

    pub async fn stream<Req: serde::Serialize, Res: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        req: &Req,
        options: &[CallOption],
    ) -> Result<Vec<Res>, SpeconnError> {
        let url = format!("{}{}", self.base_url, path);
        let body = serde_json::to_vec(req).map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?;

        let mut headers: Vec<(&str, &str)> = vec![("connect-protocol-version", "1")];
        for opt in options {
            headers.extend(opt.headers());
        }

        let resp = self.transport.post(
            &url,
            "application/connect+json",
            &body,
            &headers,
        ).await?;

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
