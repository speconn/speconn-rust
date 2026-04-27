use crate::{SpeconnError, Code, encode_envelope, FLAG_END_STREAM};
use std::collections::HashMap;
use serde_json::Value;

type UnaryHandler = Box<dyn Fn(Value) -> Result<Value, SpeconnError> + Send + Sync>;
use std::sync::{Arc, Mutex};

type StreamHandler = Box<dyn Fn(Value, Box<dyn Fn(Value) + Send + Sync>) -> Result<(), SpeconnError> + Send + Sync>;

pub struct UnaryRoute {
    pub path: String,
    pub handler: UnaryHandler,
}

pub struct StreamRoute {
    pub path: String,
    pub handler: StreamHandler,
}

pub struct SpeconnRouter {
    unary_routes: HashMap<String, UnaryHandler>,
    stream_routes: HashMap<String, StreamHandler>,
}

impl SpeconnRouter {
    pub fn new() -> Self {
        SpeconnRouter {
            unary_routes: HashMap::new(),
            stream_routes: HashMap::new(),
        }
    }

    pub fn unary<Req, Res, F>(mut self, path: &str, f: F) -> Self
    where
        Req: serde::de::DeserializeOwned + Send + Sync + 'static,
        Res: serde::Serialize + Send + Sync + 'static,
        F: Fn(Req) -> Result<Res, SpeconnError> + Send + Sync + 'static,
    {
        self.unary_routes.insert(
            path.to_string(),
            Box::new(move |v: Value| {
                let req: Req = serde_json::from_value(v)
                    .map_err(|e| SpeconnError::new(Code::InvalidArgument, e.to_string()))?;
                let res = f(req)?;
                serde_json::to_value(res)
                    .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))
            }),
        );
        self
    }

    pub fn server_stream<Req, Res, F>(mut self, path: &str, f: F) -> Self
    where
        Req: serde::de::DeserializeOwned + Send + Sync + 'static,
        Res: serde::Serialize + Send + Sync + 'static,
        F: Fn(Req, Box<dyn Fn(Res) + Send + Sync>) -> Result<(), SpeconnError> + Send + Sync + 'static,
    {
        self.stream_routes.insert(
            path.to_string(),
            Box::new(move |v: Value, send: Box<dyn Fn(Value) + Send + Sync>| {
                let req: Req = serde_json::from_value(v)
                    .map_err(|e| SpeconnError::new(Code::InvalidArgument, e.to_string()))?;
                let typed_send: Box<dyn Fn(Res) + Send + Sync> = Box::new(move |msg: Res| {
                    let val = serde_json::to_value(msg).unwrap();
                    send(val);
                });
                f(req, typed_send)
            }),
        );
        self
    }

    pub fn handle(&self, path: &str, content_type: &str, body: &[u8]) -> (u16, String, Vec<u8>) {
        if content_type.contains("connect+json") {
            if let Some(handler) = self.stream_routes.get(path) {
                return handle_stream(handler, body);
            }
        }

        if let Some(handler) = self.unary_routes.get(path) {
            return handle_unary(handler, body);
        }

        let (status, body) = json_error(Code::NotFound, &format!("no route: {}", path));
        (status, "application/json".to_string(), body)
    }
}

impl Default for SpeconnRouter {
    fn default() -> Self {
        Self::new()
    }
}

fn handle_unary(handler: &UnaryHandler, body: &[u8]) -> (u16, String, Vec<u8>) {
    let value: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => {
            let (status, body) = json_error(Code::InvalidArgument, &e.to_string());
            return (status, "application/json".to_string(), body);
        }
    };

    match handler(value) {
        Ok(res) => {
            let body = serde_json::to_vec(&res).unwrap();
            (200, "application/json".to_string(), body)
        }
        Err(e) => {
            let status = e.http_status();
            let body = serde_json::to_vec(&serde_json::json!({"code": e.code.as_str(), "message": &e.message})).unwrap();
            (status, "application/json".to_string(), body)
        }
    }
}

fn handle_stream(handler: &StreamHandler, body: &[u8]) -> (u16, String, Vec<u8>) {
    let value: Value = serde_json::from_slice(body).unwrap_or(Value::Object(serde_json::Map::new()));

    let chunks: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
    let chunks_clone = chunks.clone();

    let send: Box<dyn Fn(Value) + Send + Sync> = Box::new(move |msg: Value| {
        let payload = serde_json::to_vec(&msg).unwrap();
        chunks_clone.lock().unwrap().push(encode_envelope(0, &payload));
    });

    match handler(value, send) {
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
    for chunk in chunks {
        body.extend_from_slice(&chunk);
    }

    (200, "application/connect+json".to_string(), body)
}

fn json_error(code: Code, message: &str) -> (u16, Vec<u8>) {
    let status = code.http_status();
    let body = serde_json::to_vec(&serde_json::json!({"code": code.as_str(), "message": message})).unwrap();
    (status, body)
}
