use crate::ROUTER;

#[uniffi::export]
pub fn call(path: String, data: Vec<String>) -> String {
    let routes = &ROUTER.routes;

    match routes.get(&path) {
        Some(handler) => {
            handler.call(data)
        },
        None => "Unknown function".to_string(),
    }
}