use crate::envelope::{decode_envelope, FLAG_END_STREAM};
use crate::error::{Code, SpeconnError};
use crate::transport::{HttpResponse, SpeconnTransport};

pub struct SpeconnClient<T: SpeconnTransport> {
    base_url: String,
    path: String,
    transport: T,
}

impl<T: SpeconnTransport> SpeconnClient<T> {
    pub fn new(base_url: &str, path: &str, transport: T) -> Self {
        SpeconnClient {
            base_url: base_url.trim_end_matches('/').to_string(),
            path: path.to_string(),
            transport,
        }
    }

    pub async fn call<Req: serde::Serialize, Res: serde::de::DeserializeOwned>(
        &self,
        req: Req,
        headers: &[(&str, &str)],
    ) -> Result<Res, SpeconnError> {
        let url = format!("{}{}", self.base_url, self.path);
        let body = serde_json::to_vec(&req).unwrap_or_default();
        let mut h: Vec<(&str, &str)> = vec![("content-type", "application/json")];
        h.extend_from_iter(headers.iter().copied());
        let resp = self.transport.post(&url, &h, body).await?;
        parse_unary_response::<Res>(resp)
    }

    pub async fn stream<Req: serde::Serialize, Res: serde::de::DeserializeOwned>(
        &self,
        req: Req,
        headers: &[(&str, &str)],
    ) -> Result<Vec<Res>, SpeconnError> {
        let url = format!("{}{}", self.base_url, self.path);
        let body = serde_json::to_vec(&req).unwrap_or_default();
        let h: Vec<(&str, &str)> = vec![
            ("content-type", "application/connect+json"),
            ("connect-protocol-version", "1"),
        ];
        let mut all_headers: Vec<(&str, &str)> = h;
        all_headers.extend_from_iter(headers.iter().copied());
        let resp = self.transport.post(&url, &all_headers, body).await?;
        parse_stream_response::<Res>(resp)
    }
}

#[cfg(feature = "reqwest")]
impl SpeconnClient<crate::transport::ReqwestTransport> {
    pub fn new_default(base_url: &str, path: &str) -> Self {
        Self::new(base_url, path, crate::transport::ReqwestTransport::new())
    }
}

fn parse_unary_response<Res: serde::de::DeserializeOwned>(resp: HttpResponse) -> Result<Res, SpeconnError> {
    if resp.status >= 400 {
        let err: serde_json::Value = serde_json::from_slice(&resp.body).unwrap_or(serde_json::json!({}));
        return Err(SpeconnError::new(
            Code::from_str(err["code"].as_str().unwrap_or("unknown")),
            err["message"].as_str().unwrap_or("").to_string(),
        ));
    }
    serde_json::from_slice(&resp.body).map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))
}

fn parse_stream_response<Res: serde::de::DeserializeOwned>(resp: HttpResponse) -> Result<Vec<Res>, SpeconnError> {
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
