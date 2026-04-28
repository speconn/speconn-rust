use crate::{Code, SpeconnError, encode_envelope, FLAG_END_STREAM};
use serde_json::Value;
use std::collections::HashMap;

type UnaryHandler = Box<dyn Fn(Value) -> Result<Value, SpeconnError> + Send + Sync>;
type StreamHandler = Box<dyn Fn(Value, Box<dyn Fn(Value) + Send + Sync>) -> Result<(), SpeconnError> + Send + Sync>;

pub struct RouterResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
    pub headers: Vec<(String, String)>,
}

pub struct SpeconnRequest {
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Value,
    pub content_type: String,
    pub values: HashMap<String, String>,
}

pub trait Interceptor: Send + Sync {
    fn before(&self, req: &mut SpeconnRequest) -> Result<(), SpeconnError>;
    fn after(&self, _req: &SpeconnRequest, _resp: &mut RouterResponse) {}
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

    pub fn unary<Req, Res, F>(mut self, path: &str, f: F) -> Self
    where
        Req: serde::de::DeserializeOwned + Send + Sync + 'static,
        Res: serde::Serialize + Send + Sync + 'static,
        F: Fn(Req) -> Result<Res, SpeconnError> + Send + Sync + 'static,
    {
        self.unary_routes.insert(path.to_string(), Box::new(move |v: Value| {
            let req: Req = serde_json::from_value(v)
                .map_err(|e| SpeconnError::new(Code::InvalidArgument, e.to_string()))?;
            let res = f(req)?;
            serde_json::to_value(res).map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))
        }));
        self
    }

    pub fn server_stream<Req, Res, F>(mut self, path: &str, f: F) -> Self
    where
        Req: serde::de::DeserializeOwned + Send + Sync + 'static,
        Res: serde::Serialize + Send + Sync + 'static,
        F: Fn(Req, Box<dyn Fn(Res) + Send + Sync>) -> Result<(), SpeconnError> + Send + Sync + 'static,
    {
        self.stream_routes.insert(path.to_string(), Box::new(move |v: Value, send: Box<dyn Fn(Value) + Send + Sync>| {
            let req: Req = serde_json::from_value(v)
                .map_err(|e| SpeconnError::new(Code::InvalidArgument, e.to_string()))?;
            let typed_send: Box<dyn Fn(Res) + Send + Sync> = Box::new(move |msg: Res| {
                send(serde_json::to_value(msg).unwrap());
            });
            f(req, typed_send)
        }));
        self
    }

    pub fn handle(
        &self,
        path: &str,
        content_type: &str,
        body: &[u8],
        headers: &HashMap<String, String>,
    ) -> RouterResponse {
        let value: Value = serde_json::from_slice(body).unwrap_or(Value::Object(serde_json::Map::new()));

        let mut req = SpeconnRequest {
            path: path.to_string(),
            headers: headers.clone(),
            body: value,
            content_type: content_type.to_string(),
            values: HashMap::new(),
        };

        for interceptor in &self.interceptors {
            if let Err(e) = interceptor.before(&mut req) {
                return json_error(e.code, &e.message);
            }
        }

        let is_stream = req.content_type.contains("connect+json");
        let route_path = &req.path;

        let mut resp = if is_stream {
            if let Some(handler) = self.stream_routes.get(route_path) {
                handle_stream(handler, &req.body)
            } else if let Some(handler) = self.unary_routes.get(route_path) {
                handle_unary(handler, &req.body)
            } else {
                json_error(Code::NotFound, &format!("no route: {}", route_path))
            }
        } else if let Some(handler) = self.unary_routes.get(route_path) {
            handle_unary(handler, &req.body)
        } else {
            json_error(Code::NotFound, &format!("no route: {}", route_path))
        };

        for interceptor in &self.interceptors {
            interceptor.after(&req, &mut resp);
        }

        resp
    }
}

impl Default for SpeconnRouter {
    fn default() -> Self { Self::new() }
}

fn handle_unary(handler: &UnaryHandler, body: &Value) -> RouterResponse {
    match handler(body.clone()) {
        Ok(res) => {
            let body = serde_json::to_vec(&res).unwrap();
            RouterResponse { status: 200, content_type: "application/json".to_string(), body, headers: Vec::new() }
        }
        Err(e) => json_error(e.code, &e.message),
    }
}

fn handle_stream(handler: &StreamHandler, body: &Value) -> RouterResponse {
    use std::sync::{Arc, Mutex};
    let chunks: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
    let chunks_clone = chunks.clone();
    let send: Box<dyn Fn(Value) + Send + Sync> = Box::new(move |msg: Value| {
        let payload = serde_json::to_vec(&msg).unwrap();
        chunks_clone.lock().unwrap().push(encode_envelope(0, &payload));
    });
    match handler(body.clone(), send) {
        Ok(()) => {
            let trailer = serde_json::json!({});
            chunks.lock().unwrap().push(encode_envelope(FLAG_END_STREAM, &serde_json::to_vec(&trailer).unwrap()));
        }
        Err(e) => {
            let trailer = serde_json::json!({"error": {"code": e.code.as_str(), "message": &e.message}});
            chunks.lock().unwrap().push(encode_envelope(FLAG_END_STREAM, &serde_json::to_vec(&trailer).unwrap()));
        }
    }
    let chunks = Arc::try_unwrap(chunks).unwrap().into_inner().unwrap();
    let mut body = Vec::new();
    for chunk in chunks { body.extend_from_slice(&chunk); }
    RouterResponse { status: 200, content_type: "application/connect+json".to_string(), body, headers: Vec::new() }
}

fn json_error(code: Code, message: &str) -> RouterResponse {
    let status = code.http_status();
    let body = serde_json::to_vec(&serde_json::json!({"code": code.as_str(), "message": message})).unwrap();
    RouterResponse { status, content_type: "application/json".to_string(), body, headers: Vec::new() }
}
