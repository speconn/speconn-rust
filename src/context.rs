use std::collections::HashMap;
use std::sync::Mutex;
use tokio_util::CancellationToken;

pub struct SpeconnContext {
    pub headers: HashMap<String, String>,
    pub response_headers: Mutex<HashMap<String, String>>,
    pub response_trailers: Mutex<HashMap<String, String>>,
    pub signal: CancellationToken,
    pub method_name: String,
    pub local_addr: Option<String>,
    pub remote_addr: Option<String>,
    pub values: Mutex<HashMap<String, Box<dyn std::any::Any + Send + Sync>>>,
    headers_sent: Mutex<bool>,
}

impl SpeconnContext {
    pub fn new(
        headers: HashMap<String, String>,
        method_name: String,
        local_addr: Option<String>,
        remote_addr: Option<String>,
        timeout_ms: Option<u32>,
    ) -> Self {
        let normalized_headers: HashMap<String, String> = headers
            .into_iter()
            .map(|(k, v)| (k.to_lowercase(), v))
            .collect();

        let signal = CancellationToken::new();
        
        if let Some(timeout) = timeout_ms {
            if timeout > 0 {
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(timeout as u64)).await;
                    signal.cancel();
                });
            }
        }

        SpeconnContext {
            headers: normalized_headers,
            response_headers: Mutex::new(HashMap::new()),
            response_trailers: Mutex::new(HashMap::new()),
            signal,
            method_name,
            local_addr,
            remote_addr,
            values: Mutex::new(HashMap::new()),
            headers_sent: Mutex::new(false),
        }
    }

    pub fn set_response_header(&self, key: &str, value: &str) -> Result<(), String> {
        let sent = self.headers_sent.lock().unwrap();
        if *sent {
            return Err("headers already sent".to_string());
        }
        drop(sent);
        
        let mut headers = self.response_headers.lock().unwrap();
        headers.insert(key.to_lowercase(), value.to_string());
        Ok(())
    }

    pub fn add_response_header(&self, key: &str, value: &str) -> Result<(), String> {
        let sent = self.headers_sent.lock().unwrap();
        if *sent {
            return Err("headers already sent".to_string());
        }
        drop(sent);
        
        let mut headers = self.response_headers.lock().unwrap();
        let normalized_key = key.to_lowercase();
        if let Some(existing) = headers.get(&normalized_key) {
            headers.insert(normalized_key, format!("{}, {}", existing, value));
        } else {
            headers.insert(normalized_key, value.to_string());
        }
        Ok(())
    }

    pub fn set_response_trailer(&self, key: &str, value: &str) {
        let mut trailers = self.response_trailers.lock().unwrap();
        trailers.insert(key.to_lowercase(), value.to_string());
    }

    pub fn mark_headers_sent(&self) {
        let mut sent = self.headers_sent.lock().unwrap();
        *sent = true;
    }

    pub fn is_cancelled(&self) -> bool {
        self.signal.is_cancelled()
    }

    pub fn cleanup(&self) {
        self.signal.cancel();
    }
}

impl Drop for SpeconnContext {
    fn drop(&mut self) {
        self.cleanup();
    }
}