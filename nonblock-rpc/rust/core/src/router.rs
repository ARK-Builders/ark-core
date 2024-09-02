use std::collections::HashMap;

use serde::Serialize;

pub struct Router {
    pub routes: HashMap<String, Box<dyn Handler + 'static + Send + Sync>>,
}

impl Router {
    pub fn new() -> Self {
        Router {
            routes: HashMap::new(),
        }
    }

    pub fn add(mut self, name: &str, handler: impl Handler + 'static + Send + Sync) -> Self {
        self.routes.insert(name.to_string(), Box::new(handler));
        self
    }

    pub fn call(&self, name: &str, args: Vec<String>) -> String {
        match self.routes.get(name) {
            Some(handler) => handler.call(args),
            None => "Unknown function".to_string(),
        }
    }
}

#[derive(Serialize)]
pub struct Response<T> {
    pub result: Option<T>,
    pub error: Option<String>,
    pub is_success: bool,
}

const CATASTROPHIC_ERROR: &str = "{\"result\": null, \"error\": \"CATASTROPHIC_ERROR: Failed to serialize response\", \"is_success\": false}";

impl<T> Response<T> {
    pub fn success(result: T) -> Self {
        Response {
            result: Some(result),
            error: None,
            is_success: true,
        }
    }

    pub fn error(error: String) -> Self {
        Response {
            result: None,
            error: Some(error),
            is_success: false,
        }
    }
}

pub trait Handler {
    fn call(&self, args: Vec<String>) -> String;
}

impl<R, T0> Handler for fn(T0) -> R
where
    R: serde::Serialize,
    T0: for<'a> serde::Deserialize<'a>,
{
    fn call(&self, args: Vec<String>) -> String {
        let response = {
            let arg0 = serde_json::from_str::<T0>(&args[0]);
            match arg0 {
                Ok(arg0) => Response::success((self)(arg0)),
                Err(_) => Response::error(format!("Failed to deserialize argument at position 0")),
            }
        };

        serde_json::to_string(&response).unwrap_or(CATASTROPHIC_ERROR.into())
    }
}

impl<R, T0, T1> Handler for fn(T0, T1) -> R
where
    R: serde::Serialize,
    T0: for<'a> serde::Deserialize<'a>,
    T1: for<'a> serde::Deserialize<'a>,
{
    fn call(&self, args: Vec<String>) -> String {
        let response = {
            let arg0 = serde_json::from_str::<T0>(&args[0]);
            let arg1 = serde_json::from_str::<T1>(&args[1]);
            match (arg0, arg1) {
                (Ok(arg0), Ok(arg1)) => Response::success((self)(arg0, arg1)),
                _ => Response::error(format!("Failed to deserialize arguments")),
            }
        };
        serde_json::to_string(&response).unwrap_or(CATASTROPHIC_ERROR.into())
    }
}