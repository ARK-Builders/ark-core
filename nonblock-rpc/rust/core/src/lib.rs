use once_cell::sync::Lazy;
use router::Router;
use std::collections::BTreeMap;

uniffi::setup_scaffolding!();

mod api;
mod router;
mod rpc;

pub static ROUTER: Lazy<Router> = Lazy::new(|| {
    Router::new()
        .add("factorial", factorial as fn(u64) -> u64)
        .add("sum", sum as fn(Vec<u64>, u64) -> u64)
});

pub fn factorial(payload: u64) -> u64 {
    let mut result = 1;
    for i in 1..=payload {
        result *= i;
    }
    result
}

pub fn sum(arg1: Vec<u64>, arg2: u64) -> u64 {
    arg1.iter().sum::<u64>() + arg2
}


#[uniffi::export]
pub fn my_func(map: BTreeMap<u64, String>) -> String {
    let mut result = 0;
    for (key, value) in map {
        result += value;
    }
    result.to_string()
}