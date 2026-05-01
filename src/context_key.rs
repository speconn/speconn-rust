use std::any::Any;
use std::sync::Arc;
use super::SpeconnContext;

pub struct ContextKey<T: Any + Send + Sync + Clone> {
    pub id: String,
    pub default_value: T,
}

impl<T: Any + Send + Sync + Clone> ContextKey<T> {
    pub fn new(id: String, default_value: T) -> Self {
        ContextKey { id, default_value }
    }
}

pub fn set_value<T: Any + Send + Sync + Clone>(ctx: &SpeconnContext, key: &ContextKey<T>, value: T) {
    let mut values = ctx.values.lock().unwrap();
    values.insert(key.id.clone(), Box::new(value));
}

pub fn get_value<T: Any + Send + Sync + Clone>(ctx: &SpeconnContext, key: &ContextKey<T>) -> T {
    let values = ctx.values.lock().unwrap();
    if let Some(v) = values.get(&key.id) {
        if let Some(typed) = v.downcast_ref::<T>() {
            return typed.clone();
        }
    }
    key.default_value.clone()
}

pub fn delete_value<T: Any + Send + Sync + Clone>(ctx: &SpeconnContext, key: &ContextKey<T>) {
    let mut values = ctx.values.lock().unwrap();
    values.remove(&key.id);
}

pub fn get_user(ctx: &SpeconnContext) -> String {
    let values = ctx.values.lock().unwrap();
    if let Some(v) = values.get("user") {
        if let Some(user) = v.downcast_ref::<String>() {
            return user.clone();
        }
    }
    String::new()
}

pub fn set_user(ctx: &SpeconnContext, user: String) {
    let mut values = ctx.values.lock().unwrap();
    values.insert("user".to_string(), Box::new(user));
}

pub fn get_request_id(ctx: &SpeconnContext) -> String {
    let values = ctx.values.lock().unwrap();
    if let Some(v) = values.get("request-id") {
        if let Some(id) = v.downcast_ref::<String>() {
            return id.clone();
        }
    }
    String::new()
}

pub fn set_request_id(ctx: &SpeconnContext, id: String) {
    let mut values = ctx.values.lock().unwrap();
    values.insert("request-id".to_string(), Box::new(id));
}

pub fn user_key() -> ContextKey<String> {
    ContextKey::new("user".to_string(), String::new())
}

pub fn request_id_key() -> ContextKey<String> {
    ContextKey::new("request-id".to_string(), String::new())
}

pub fn user_id_key() -> ContextKey<i64> {
    ContextKey::new("user-id".to_string(), 0)
}