use crate::{Code, SpeconnError, encode_envelope, FLAG_END_STREAM};
use crate::context::SpeconnContext;
use crate::context_key::{set_user, set_request_id};
use specodec::{SpecCodec, dispatch, respond};

fn extract_format(mime: &str) -> &str {
    if mime.contains("msgpack") { "msgpack" } else { "json" }
}

fn format_to_mime(fmt: &str, stream: bool) -> String {
    let base = if fmt == "msgpack" { "msgpack" } else { "json" };
    if stream { format!("application/connect+{}", base) } else { format!("application/{}", base) }
}
use std::collections::HashMap;

type UnaryHandler = Box<dyn Fn(&SpeconnContext, &[u8], &str, &str) -> Result<Vec<u8>, SpeconnError> + Send + Sync>;
type StreamHandler = Box<dyn Fn(&SpeconnContext, &[u8], &str, &str, Box<dyn Fn(&[u8]) + Send + Sync>) -> Result<(), SpeconnError> + Send + Sync>;

pub struct RouterResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
    pub headers: Vec<(String, String)>,
}

pub struct SpeconnRequest {
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub content_type: String,
    pub accept: String,
}

pub trait Interceptor: Send + Sync {
    fn before(&self, ctx: &SpeconnContext, req: &SpeconnRequest) -> Result<(), SpeconnError>;
    fn after(&self, ctx: &SpeconnContext, resp: &mut RouterResponse) {}
}

pub struct SpeconnRouter {
    unary_routes: HashMap<String, UnaryHandler>,
    stream_routes: HashMap<String, StreamHandler>,
    interceptors: Vec<Box<dyn Interceptor>>,
}

impl SpeconnRouter {
    pub fn new() -> Self {
        SpeconnRouter {
            unary_routes: HashMap::new(),
            stream_routes: HashMap::new(),
            interceptors: Vec::new(),
        }
    }

    pub fn with_interceptor(mut self, i: Box<dyn Interceptor>) -> Self {
        self.interceptors.push(i);
        self
    }

    pub fn unary<Treq, Tres, F>(mut self, path: &str, req_codec: SpecCodec<Treq>, res_codec: SpecCodec<Tres>, f: F) -> Self
    where
        Treq: Send + Sync + 'static,
        Tres: Send + Sync + 'static,
        F: Fn(&SpeconnContext, Treq) -> Result<Tres, SpeconnError> + Send + Sync + 'static,
    {
        self.unary_routes.insert(path.to_string(), Box::new(move |ctx: &SpeconnContext, body: &[u8], ct: &str, accept: &str| {
            let req = dispatch(&req_codec, body, extract_format(ct))
                .map_err(|e| SpeconnError::new(Code::InvalidArgument, e.to_string()))?;
            let res = f(ctx, req)?;
            Ok(respond(&res_codec, &res, extract_format(accept)).body)
        }));
        self
    }

    pub fn server_stream<Treq, Tres, F>(mut self, path: &str, req_codec: SpecCodec<Treq>, res_codec: SpecCodec<Tres>, f: F) -> Self
    where
        Treq: Send + Sync + 'static,
        Tres: Send + Sync + 'static,
        F: Fn(&SpeconnContext, Treq, Box<dyn Fn(Tres) + Send + Sync>) -> Result<(), SpeconnError> + Send + Sync + 'static,
    {
        self.stream_routes.insert(path.to_string(), Box::new(move |ctx: &SpeconnContext, body: &[u8], ct: &str, accept: &str, send: Box<dyn Fn(&[u8]) + Send + Sync>| {
            let res_fmt = extract_format(accept).to_string();
            let req = dispatch(&req_codec, body, extract_format(ct))
                .map_err(|e| SpeconnError::new(Code::InvalidArgument, e.to_string()))?;
            let typed_send: Box<dyn Fn(Tres) + Send + Sync> = Box::new(move |msg: Tres| {
                send(&respond(&res_codec, &msg, &res_fmt).body);
            });
            f(ctx, req, typed_send)
        }));
        self
    }

    pub fn handle(
        &self,
        path: &str,
        content_type: &str,
        accept: &str,
        body: &[u8],
        headers: &HashMap<String, String>,
    ) -> RouterResponse {
        let timeout_ms: Option<u32> = headers
            .get("speconn-timeout-ms")
            .and_then(|s| s.parse::<u32>().ok());

        let ctx = SpeconnContext::new(
            headers.clone(),
            path.to_string(),
            None,
            None,
            timeout_ms,
        );

        let req = SpeconnRequest {
            path: path.to_string(),
            headers: headers.clone(),
            body: body.to_vec(),
            content_type: content_type.to_string(),
            accept: accept.to_string(),
        };

        for interceptor in &self.interceptors {
            if let Err(e) = interceptor.before(&ctx, &req) {
                ctx.cleanup();
                return json_error(e.code, &e.message);
            }
        }

        let is_stream = req.content_type.contains("connect+");
        let route_path = &req.path;

        let mut resp = if is_stream {
            if let Some(handler) = self.stream_routes.get(route_path) {
                handle_stream(handler, &ctx, &req.body, &req.content_type, &req.accept)
            } else if let Some(handler) = self.unary_routes.get(route_path) {
                handle_unary(handler, &ctx, &req.body, &req.content_type, &req.accept)
            } else {
                json_error(Code::NotFound, &format!("no route: {}", route_path))
            }
        } else if let Some(handler) = self.unary_routes.get(route_path) {
            handle_unary(handler, &ctx, &req.body, &req.content_type, &req.accept)
        } else {
            json_error(Code::NotFound, &format!("no route: {}", route_path))
        };

        {
            let response_headers = ctx.response_headers.lock().unwrap();
            for (k, v) in response_headers.iter() {
                resp.headers.push((k.clone(), v.clone()));
            }
        }

        for interceptor in &self.interceptors {
            interceptor.after(&ctx, &mut resp);
        }

        ctx.cleanup();
        resp
    }
}

impl Default for SpeconnRouter {
    fn default() -> Self { Self::new() }
}

fn handle_unary(handler: &UnaryHandler, ctx: &SpeconnContext, body: &[u8], ct: &str, accept: &str) -> RouterResponse {
    match handler(ctx, body, ct, accept) {
        Ok(res) => {
            let resp_ct = format_to_mime(extract_format(accept), false);
            let mut headers = Vec::new();
            {
                let response_headers = ctx.response_headers.lock().unwrap();
                for (k, v) in response_headers.iter() {
                    headers.push((k.clone(), v.clone()));
                }
            }
            RouterResponse { status: 200, content_type: resp_ct, body: res, headers }
        }
        Err(e) => json_error(e.code, &e.message),
    }
}

fn handle_stream(handler: &StreamHandler, ctx: &SpeconnContext, body: &[u8], ct: &str, accept: &str) -> RouterResponse {
    use std::sync::{Arc, Mutex};
    
    {
        let response_headers = ctx.response_headers.lock().unwrap();
        if !response_headers.is_empty() {
            ctx.mark_headers_sent();
        }
    }
    
    let chunks: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
    let chunks_clone = chunks.clone();
    let send: Box<dyn Fn(&[u8]) + Send + Sync> = Box::new(move |payload: &[u8]| {
        chunks_clone.lock().unwrap().push(encode_envelope(0, payload));
    });
    
    match handler(ctx, body, ct, accept, send) {
        Ok(()) => {
            chunks.lock().unwrap().push(encode_envelope(FLAG_END_STREAM, &[]));
        }
        Err(e) => {
            let payload = e.encode(extract_format(accept));
            chunks.lock().unwrap().push(encode_envelope(FLAG_END_STREAM, &payload));
        }
    }
    
    ctx.mark_headers_sent();
    
    let chunks = Arc::try_unwrap(chunks).unwrap().into_inner().unwrap();
    let mut body = Vec::new();
    for chunk in chunks { body.extend_from_slice(&chunk); }
    let resp_ct = format_to_mime(extract_format(accept), true);
    
    let mut headers = Vec::new();
    {
        let response_headers = ctx.response_headers.lock().unwrap();
        for (k, v) in response_headers.iter() {
            headers.push((k.clone(), v.clone()));
        }
    }
    
    RouterResponse { status: 200, content_type: resp_ct, body, headers }
}

fn json_error(code: Code, message: &str) -> RouterResponse {
    let status = code.http_status();
    let body = SpeconnError::new(code, message).encode("json");
    RouterResponse { status, content_type: "application/json".to_string(), body, headers: Vec::new() }
}