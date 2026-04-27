use crate::{SpeconnError, Code, decode_envelope, FLAG_END_STREAM};

#[cfg(feature = "client")]
pub struct SpeconnClient {
    base_url: String,
    client: reqwest::Client,
}

#[cfg(feature = "client")]
impl SpeconnClient {
    pub fn new(base_url: &str) -> Self {
        SpeconnClient {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn call_unary<Req: serde::Serialize, Res: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        req: &Req,
    ) -> Result<Res, SpeconnError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client
            .post(&url)
            .header("content-type", "application/json")
            .json(req)
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let err: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::json!({}));
            return Err(SpeconnError::new(
                Code::from_str(err["code"].as_str().unwrap_or("unknown")),
                err["message"].as_str().unwrap_or(&body).to_string(),
            ));
        }

        resp.json::<Res>().await
            .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))
    }

    pub async fn call_server_stream<Req: serde::Serialize, Res: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        req: &Req,
    ) -> Result<Vec<Res>, SpeconnError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client
            .post(&url)
            .header("content-type", "application/connect+json")
            .header("connect-protocol-version", "1")
            .json(req)
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let err: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::json!({}));
            return Err(SpeconnError::new(
                Code::from_str(err["code"].as_str().unwrap_or("unknown")),
                err["message"].as_str().unwrap_or(&body).to_string(),
            ));
        }

        let resp_body = resp.bytes().await
            .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?;

        let mut results = Vec::new();
        let mut pos = 0;

        while pos < resp_body.len() {
            if resp_body.len() - pos < 5 {
                break;
            }
            let (flags, payload) = decode_envelope(&resp_body[pos..])
                .map_err(|e| SpeconnError::new(Code::Internal, e))?;
            pos += 5 + payload.len();

            if flags & FLAG_END_STREAM != 0 {
                let trailer: serde_json::Value = serde_json::from_slice(payload)
                    .unwrap_or(serde_json::json!({}));
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

#[cfg(feature = "client")]
fn map_reqwest_error(e: reqwest::Error) -> SpeconnError {
    SpeconnError::new(Code::Unavailable, e.to_string())
}

impl Code {
    pub fn from_str(s: &str) -> Self {
        match s {
            "canceled" => Self::Canceled,
            "unknown" => Self::Unknown,
            "invalid_argument" => Self::InvalidArgument,
            "deadline_exceeded" => Self::DeadlineExceeded,
            "not_found" => Self::NotFound,
            "already_exists" => Self::AlreadyExists,
            "permission_denied" => Self::PermissionDenied,
            "resource_exhausted" => Self::ResourceExhausted,
            "failed_precondition" => Self::FailedPrecondition,
            "aborted" => Self::Aborted,
            "out_of_range" => Self::OutOfRange,
            "unimplemented" => Self::Unimplemented,
            "internal" => Self::Internal,
            "unavailable" => Self::Unavailable,
            "data_loss" => Self::DataLoss,
            "unauthenticated" => Self::Unauthenticated,
            _ => Self::Unknown,
        }
    }
}
