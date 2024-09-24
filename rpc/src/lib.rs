pub mod router;
pub use once_cell;

#[macro_export]
macro_rules! uniffi_rpc_server {
    ($($name:ident),*) => {
        pub static ROUTER: rpc::once_cell::sync::Lazy<Router> = rpc::once_cell::sync::Lazy::new(|| {
            let mut router = Router::new();
            $(
                router.add(stringify!($name), $name);
            )*
            router
        });

        #[uniffi::export]
        pub fn call(path: String, data: Vec<String>) -> String {
            ROUTER.call(&path, data)
        }

        #[uniffi::export]
        pub async fn call_async(path:String,data:Vec<String>) -> String {
            ROUTER.call(&path,data)
        }
    };
}