use crate::{SpeconnError, Code, decode_envelope, FLAG_END_STREAM};

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

/// Parse a Connect streaming response body into individual messages.
/// Returns a vector of deserialized messages and optionally an error from the trailer.
pub fn parse_stream<Res: serde::de::DeserializeOwned>(body: &[u8]) -> Result<Vec<Res>, SpeconnError> {
    let mut results = Vec::new();
    let mut pos = 0;

    while pos < body.len() {
        if body.len() - pos < 5 {
            break;
        }
        let (flags, payload) = decode_envelope(&body[pos..])
            .map_err(|e| SpeconnError::new(Code::Internal, e))?;
        let frame_len = 5 + payload.len();
        pos += frame_len;

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
