use crate::envelope::{decode_envelope, FLAG_END_STREAM};
use crate::error::{Code, SpeconnError};
use crate::transport::{HttpResponse, SpeconnTransport};
use specodec::{SpecCodec, dispatch, respond};
use std::collections::HashMap;

fn get_content_type<'a>(headers: &'a HashMap<&str, &'a str>) -> &'a str {
    headers.get("content-type").copied().unwrap_or("application/json")
}

fn get_accept<'a>(headers: &'a HashMap<&str, &'a str>) -> &'a str {
    headers.get("accept").copied().unwrap_or_else(|| get_content_type(headers))
}

fn extract_format(mime: &str) -> &str {
    if mime.contains("msgpack") { "msgpack" } else { "json" }
}

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

    pub async fn call<Treq, Tres>(
        &self,
        req_codec: &SpecCodec<Treq>,
        req: &Treq,
        res_codec: &SpecCodec<Tres>,
        headers: HashMap<&str, &str>,
    ) -> Result<Tres, SpeconnError> {
        let url = format!("{}{}", self.base_url, self.path);
        let content_type = get_content_type(&headers);
        let accept = get_accept(&headers);
        let req_fmt = extract_format(content_type);
        let res_fmt = extract_format(accept);

        let body = respond(req_codec, req, req_fmt).body;

        let h: Vec<(&str, &str)> = headers.into_iter().collect();
        let resp = self.transport.post(&url, &h, body).await?;
        if resp.status >= 400 {
            return parse_error(&resp);
        }
        dispatch(res_codec, &resp.body, res_fmt)
            .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))
    }

    pub async fn stream<Treq, Tres>(
        &self,
        req_codec: &SpecCodec<Treq>,
        req: &Treq,
        res_codec: &SpecCodec<Tres>,
        headers: HashMap<&str, &str>,
    ) -> Result<Vec<Tres>, SpeconnError> {
        let url = format!("{}{}", self.base_url, self.path);
        let content_type = get_content_type(&headers);
        let accept = get_accept(&headers);
        let req_fmt = extract_format(content_type);
        let res_fmt = extract_format(accept);

        let body = respond(req_codec, req, req_fmt).body;

        let mut h = headers;
        if !h.contains_key("connect-protocol-version") {
            h.insert("connect-protocol-version", "1");
        }

        let req_headers: Vec<(&str, &str)> = h.into_iter().collect();
        let resp = self.transport.post(&url, &req_headers, body).await?;
        if resp.status >= 400 {
            return parse_error(&resp);
        }
        parse_stream_response(res_codec, &resp, res_fmt)
    }
}

#[cfg(feature = "reqwest")]
impl SpeconnClient<crate::transport::ReqwestTransport> {
    pub fn new_default(base_url: &str, path: &str) -> Self {
        Self::new(base_url, path, crate::transport::ReqwestTransport::new())
    }
}

fn parse_error<T>(resp: &HttpResponse) -> Result<T, SpeconnError> {
    Err(SpeconnError::decode(&resp.body, "json"))
}

fn parse_stream_response<T>(
    codec: &SpecCodec<T>,
    resp: &HttpResponse,
    res_fmt: &str,
) -> Result<Vec<T>, SpeconnError> {
    let mut results = Vec::new();
    let mut pos = 0;
    while pos < resp.body.len() {
        if resp.body.len() - pos < 5 { break; }
        let (flags, payload) = decode_envelope(&resp.body[pos..])
            .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?;
        pos += 5 + payload.len();
        if flags & FLAG_END_STREAM != 0 {
            if !payload.is_empty() {
                return Err(SpeconnError::decode(payload, res_fmt));
            }
            break;
        }
        let obj = dispatch(codec, payload, res_fmt)
            .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?;
        results.push(obj);
    }
    Ok(results)
}
