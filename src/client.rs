use crate::envelope::{decode_envelope, FLAG_END_STREAM};
use crate::error::{Code, SpeconnError};
use crate::transport::{HttpResponse, SpeconnTransport};
use futures::Stream;
use specodec::{SpecCodec, dispatch, respond};
use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;

pub struct CallOptions {
    pub headers: HashMap<String, String>,
    pub timeout_ms: Option<u64>,
}

impl CallOptions {
    pub fn new() -> Self {
        Self {
            headers: HashMap::new(),
            timeout_ms: None,
        }
    }
}

impl Default for CallOptions {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Response<T> {
    pub msg: T,
    pub headers: HashMap<String, String>,
    pub trailers: HashMap<String, String>,
}

pub struct StreamResponse<T> {
    pub headers: HashMap<String, String>,
    pub trailers: HashMap<String, String>,
    msgs: Vec<T>,
}

impl<T> StreamResponse<T> {
    pub fn as_stream(self) -> Pin<Box<dyn Stream<Item = Result<T, SpeconnError>> + Send>> {
        Box::pin(futures::stream::iter(self.msgs.into_iter().map(Ok)))
    }

    fn add_msg(&mut self, msg: T) {
        self.msgs.push(msg);
    }

    fn set_trailers(&mut self, t: HashMap<String, String>) {
        self.trailers = t;
    }
}

fn split_headers_trailers(raw_headers: Vec<(String, String)>) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut headers = HashMap::new();
    let mut trailers = HashMap::new();
    for (k, v) in raw_headers {
        if k.to_lowercase().starts_with("trailer-") {
            trailers.insert(k[8..].to_string(), v);
        } else {
            headers.insert(k.to_lowercase(), v);
        }
    }
    (headers, trailers)
}

fn get_content_type(headers: &HashMap<String, String>) -> &str {
    headers.get("content-type").map(|s| s.as_str()).unwrap_or("application/json")
}

fn get_accept(headers: &HashMap<String, String>) -> &str {
    headers.get("accept").map(|s| s.as_str()).unwrap_or_else(|| get_content_type(headers))
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
        options: CallOptions,
    ) -> Result<Response<Tres>, SpeconnError> {
        let url = format!("{}{}", self.base_url, self.path);
        let content_type = get_content_type(&options.headers);
        let accept = get_accept(&options.headers);
        let req_fmt = extract_format(content_type);
        let res_fmt = extract_format(accept);

        let body = respond(req_codec, req, req_fmt).body;

        let h: Vec<(&str, &str)> = options.headers.iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let resp = self.transport.post(&url, &h, body).await?;
        if resp.status >= 400 {
            return Err(SpeconnError::decode(&resp.body, "json"));
        }
        let (resp_headers, resp_trailers) = split_headers_trailers(resp.headers);
        let msg = dispatch(res_codec, &resp.body, res_fmt)
            .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?;
        Ok(Response {
            msg,
            headers: resp_headers,
            trailers: resp_trailers,
        })
    }

    pub async fn stream<Treq, Tres>(
        &self,
        req_codec: &SpecCodec<Treq>,
        req: &Treq,
        res_codec: &SpecCodec<Tres>,
        options: CallOptions,
    ) -> Result<StreamResponse<Tres>, SpeconnError> {
        let url = format!("{}{}", self.base_url, self.path);
        let content_type = get_content_type(&options.headers);
        let accept = get_accept(&options.headers);
        let req_fmt = extract_format(content_type);
        let res_fmt = extract_format(accept);

        let mut h = options.headers.clone();
        if !h.contains_key("connect-protocol-version") {
            h.insert("connect-protocol-version".to_string(), "1".to_string());
        }

        let body = respond(req_codec, req, req_fmt).body;

        let req_headers: Vec<(&str, &str)> = h.iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let resp = self.transport.post(&url, &req_headers, body).await?;
        if resp.status >= 400 {
            return Err(SpeconnError::decode(&resp.body, "json"));
        }

        let (resp_headers, resp_trailers) = split_headers_trailers(resp.headers);
        let mut stream_resp = StreamResponse {
            headers: resp_headers,
            trailers: HashMap::new(),
            msgs: Vec::new(),
        };

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
            let msg = dispatch(res_codec, payload, res_fmt)
                .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))?;
            stream_resp.add_msg(msg);
        }

        stream_resp.set_trailers(resp_trailers);
        Ok(stream_resp)
    }
}

#[cfg(feature = "reqwest")]
impl SpeconnClient<crate::transport::ReqwestTransport> {
    pub fn new_default(base_url: &str, path: &str) -> Self {
        Self::new(base_url, path, crate::transport::ReqwestTransport::new())
    }
}