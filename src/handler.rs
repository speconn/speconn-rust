use crate::{SpeconnError, Code, encode_envelope, FLAG_END_STREAM};
use std::future::Future;

pub trait SpeconnService: Clone + Send + Sync + 'static {
    type Request: serde::de::DeserializeOwned + Send + Sync + 'static;
    type Response: serde::Serialize + Send + Sync + 'static;

    fn path(&self) -> &str;
    fn call(&self, req: Self::Request) -> impl Future<Output = Result<Self::Response, SpeconnError>> + Send;
}

#[derive(Clone)]
pub struct UnaryRoute {
    pub path: String,
    pub handler: fn(serde_json::Value) -> Result<serde_json::Value, SpeconnError>,
}

pub fn json_handler<Req, Res, F>(path: &str, f: F) -> UnaryRoute
where
    Req: serde::de::DeserializeOwned,
    Res: serde::Serialize,
    F: Fn(Req) -> Result<Res, SpeconnError> + Send + Sync + 'static,
{
    UnaryRoute {
        path: path.to_string(),
        handler: move |v: serde_json::Value| {
            let req: Req = serde_json::from_value(v)
                .map_err(|e| SpeconnError::new(Code::InvalidArgument, e.to_string()))?;
            let res = f(req)?;
            serde_json::to_value(res)
                .map_err(|e| SpeconnError::new(Code::Internal, e.to_string()))
        },
    }
}

pub fn handle_unary(routes: &[UnaryRoute], path: &str, body: &[u8]) -> (u16, Vec<u8>) {
    let route = match routes.iter().find(|r| r.path == path) {
        Some(r) => r,
        None => {
            let err = serde_json::json!({"code": "not_found", "message": format!("no route: {}", path)});
            return (404, serde_json::to_vec(&err).unwrap());
        }
    };

    let value: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => {
            let err = serde_json::json!({"code": "invalid_argument", "message": e.to_string()});
            return (400, serde_json::to_vec(&err).unwrap());
        }
    };

    match (route.handler)(value) {
        Ok(res) => (200, serde_json::to_vec(&res).unwrap()),
        Err(e) => {
            let status = e.http_status();
            (status, serde_json::to_vec(&serde_json::json!({"code": e.code.as_str(), "message": e.message})).unwrap())
        }
    }
}

pub fn end_stream_frame(error: Option<&SpeconnError>) -> Vec<u8> {
    let trailer = match error {
        Some(e) => serde_json::json!({"error": {"code": e.code.as_str(), "message": &e.message}}),
        None => serde_json::json!({}),
    };
    let payload = serde_json::to_vec(&trailer).unwrap();
    encode_envelope(FLAG_END_STREAM, &payload)
}
