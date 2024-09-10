use std::{collections::HashMap, marker::PhantomData};

use serde::{Deserialize, Serialize};

pub struct Router {
    pub routes: HashMap<String, Box<dyn Handler + 'static + Send + Sync>>,
}

impl Router {
    pub fn new() -> Self {
        Router {
            routes: HashMap::new(),
        }
    }

    pub fn from_routes(
        routes: HashMap<String, Box<dyn Handler + 'static + Send + Sync>>,
    ) -> Self {
        Router { routes }
    }

    pub fn add<Marker: 'static + Send + Sync>(
        &mut self,
        name: &str,
        function: impl HandlerFunction<Marker>,
    ) {
        self.routes.insert(
            name.to_owned(),
            Box::new(FunctionHandler {
                function,
                marker: PhantomData,
            }),
        );
    }

    pub fn call(&self, name: &str, args: Vec<String>) -> String {
        match self.routes.get(name) {
            Some(handler) => handler.call(args),
            None => NOT_FOUND.into(),
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
const NOT_FOUND: &str = "{\"result\": null, \"error\": \"NOT_FOUND: Unknown function\", \"is_success\": false}";

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
pub trait HandlerFunction<Marker>: Send + Sync + 'static {
    fn call(&self, args: Vec<String>) -> String;
}

#[allow(non_snake_case)]
impl<F, T0, R> HandlerFunction<fn(T0) -> R> for F
where
    F: Fn(T0) -> R + Send + Sync + 'static,
    T0: for<'a> Deserialize<'a>,
    R: Serialize,
{
    fn call(&self, args: Vec<String>) -> String {
        let response = {
            let mut args = args.into_iter();
            let T0 = serde_json::from_str::<T0>(
                &args.next().unwrap_or("{}".to_string()),
            );
            match T0 {
                core::result::Result::Ok(T0) => Response::success((self)(T0)),
                _ => {
                    Response::error(format!("Failed to deserialize arguments"))
                }
            }
        };
        serde_json::to_string(&response).unwrap_or(CATASTROPHIC_ERROR.into())
    }
}

macro_rules! impl_handler_function {
    ($($type:ident),+) => {
        #[allow(non_snake_case)]
        impl<F, $($type),+, R> HandlerFunction<fn($($type),+) -> R> for F
        where
            F: Fn($($type),+) -> R + Send + Sync + 'static,
            $($type: for<'a> Deserialize<'a>,)+
            R: Serialize,
        {
            fn call(&self, args: Vec<String>) -> String {
                let response = {
                    let mut args = args.into_iter();
                    let ($($type,)*) = (
                        $(
                            serde_json::from_str::<$type>(&args.next().unwrap_or("{}".to_string()))
                        ),+
                    );
                    match ($($type,)*) {
                        ($(core::result::Result::Ok($type),)*) => Response::success((self)($($type,)*)),
                        _ => Response::error(format!("Failed to deserialize arguments")),
                    }
                };
                serde_json::to_string(&response).unwrap_or(CATASTROPHIC_ERROR.into())
            }
        }
    };
}

impl_handler_function!(T0, T1);
impl_handler_function!(T0, T1, T2);
impl_handler_function!(T0, T1, T2, T3);
impl_handler_function!(T0, T1, T2, T3, T4);

struct FunctionHandler<F, Marker> {
    function: F,
    marker: PhantomData<Marker>,
}

impl<F: HandlerFunction<Marker>, Marker> Handler
    for FunctionHandler<F, Marker>
{
    fn call(&self, args: Vec<String>) -> String {
        self.function.call(args)
    }
}

pub trait Handler {
    fn call(&self, args: Vec<String>) -> String;
}
