#[cfg(test)]
mod tests {
    use crate::context::SpeconnContext;
    use crate::context_key::{ContextKey, set_value, get_value, delete_value, user_key, request_id_key, get_user, set_user};
    use std::collections::HashMap;
    use tokio::time::{sleep, Duration};

    #[test]
    fn test_context_fields() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token".to_string());
        headers.insert("X-Custom".to_string(), "value".to_string());
        headers.insert("CONTENT-TYPE".to_string(), "application/json".to_string());

        let ctx = SpeconnContext::new(
            headers,
            "/test.Service/Method".to_string(),
            Some("localhost:8001".to_string()),
            Some("192.168.1.100:54321".to_string()),
            Some(5000),
        );

        assert_eq!(ctx.method_name, "/test.Service/Method");
        assert_eq!(ctx.local_addr, Some("localhost:8001".to_string()));
        assert_eq!(ctx.remote_addr, Some("192.168.1.100:54321".to_string()));

        assert_eq!(ctx.headers.get("authorization"), Some(&"Bearer token".to_string()));
        assert_eq!(ctx.headers.get("x-custom"), Some(&"value".to_string()));
        assert_eq!(ctx.headers.get("content-type"), Some(&"application/json".to_string()));
    }

    #[test]
    fn test_response_headers() {
        let ctx = SpeconnContext::new(
            HashMap::new(),
            "/test".to_string(),
            Some("localhost:8001".to_string()),
            Some("client:123".to_string()),
            None,
        );

        ctx.set_response_header("X-Custom", "value1").unwrap();
        let headers = ctx.response_headers.lock().unwrap();
        assert_eq!(headers.get("x-custom"), Some(&"value1".to_string()));
        drop(headers);

        ctx.set_response_header("X-Custom", "value2").unwrap();
        let headers = ctx.response_headers.lock().unwrap();
        assert_eq!(headers.get("x-custom"), Some(&"value2".to_string()));
        drop(headers);

        ctx.add_response_header("X-Multi", "v1").unwrap();
        ctx.add_response_header("X-Multi", "v2").unwrap();
        let headers = ctx.response_headers.lock().unwrap();
        assert_eq!(headers.get("x-multi"), Some(&"v1, v2".to_string()));
        drop(headers);

        ctx.mark_headers_sent();
        
        let result = ctx.set_response_header("X-Another", "value3");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "headers already sent".to_string());
    }

    #[test]
    fn test_response_trailers() {
        let ctx = SpeconnContext::new(
            HashMap::new(),
            "/test".to_string(),
            Some("localhost:8001".to_string()),
            Some("client:123".to_string()),
            None,
        );

        ctx.set_response_trailer("X-Total-Count", "100");
        ctx.set_response_trailer("X-Request-Id", "abc-123");

        let trailers = ctx.response_trailers.lock().unwrap();
        assert_eq!(trailers.get("x-total-count"), Some(&"100".to_string()));
        assert_eq!(trailers.get("x-request-id"), Some(&"abc-123".to_string()));
    }

    #[tokio::test]
    async fn test_timeout_signal() {
        let ctx = SpeconnContext::new(
            HashMap::new(),
            "/test".to_string(),
            Some("localhost:8001".to_string()),
            Some("client:123".to_string()),
            Some(100),
        );

        sleep(Duration::from_millis(150)).await;

        assert!(ctx.is_cancelled());
    }

    #[test]
    fn test_no_timeout() {
        let ctx = SpeconnContext::new(
            HashMap::new(),
            "/test".to_string(),
            Some("localhost:8001".to_string()),
            Some("client:123".to_string()),
            None,
        );

        assert!(!ctx.is_cancelled());
    }

    #[test]
    fn test_context_key_typed() {
        let ctx = SpeconnContext::new(
            HashMap::new(),
            "/test".to_string(),
            Some("localhost:8001".to_string()),
            Some("client:123".to_string()),
            None,
        );

        let test_key = ContextKey::new("test".to_string(), "default".to_string());

        set_value(&ctx, &test_key, "value1".to_string());
        let value = get_value(&ctx, &test_key);
        assert_eq!(value, "value1".to_string());

        delete_value(&ctx, &test_key);
        let default_value = get_value(&ctx, &test_key);
        assert_eq!(default_value, "default".to_string());

        let int_key = ContextKey::new("int-test".to_string(), 0);
        set_value(&ctx, &int_key, 42);
        let int_value = get_value(&ctx, &int_key);
        assert_eq!(int_value, 42);
    }

    #[test]
    fn test_predefined_keys() {
        let ctx = SpeconnContext::new(
            HashMap::new(),
            "/test".to_string(),
            Some("localhost:8001".to_string()),
            Some("client:123".to_string()),
            None,
        );

        set_user(&ctx, "alice".to_string());
        let user = get_user(&ctx);
        assert_eq!(user, "alice".to_string());

        let user_key = user_key();
        let user2 = get_value(&ctx, &user_key);
        assert_eq!(user2, "alice".to_string());

        set_user(&ctx, "bob".to_string());
        let user3 = get_user(&ctx);
        assert_eq!(user3, "bob".to_string());
    }

    #[test]
    fn test_headers_normalization() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token".to_string());
        headers.insert("CONTENT-TYPE".to_string(), "application/json".to_string());
        headers.insert("X-Custom-Header".to_string(), "value".to_string());

        let ctx = SpeconnContext::new(
            headers,
            "/test".to_string(),
            Some("localhost:8001".to_string()),
            Some("client:123".to_string()),
            None,
        );

        assert_eq!(ctx.headers.get("authorization"), Some(&"Bearer token".to_string()));
        assert_eq!(ctx.headers.get("content-type"), Some(&"application/json".to_string()));
        assert_eq!(ctx.headers.get("x-custom-header"), Some(&"value".to_string()));
    }

    #[test]
    fn test_cleanup() {
        let ctx = SpeconnContext::new(
            HashMap::new(),
            "/test".to_string(),
            Some("localhost:8001".to_string()),
            Some("client:123".to_string()),
            Some(1000),
        );

        ctx.cleanup();

        assert!(ctx.is_cancelled());
    }
}