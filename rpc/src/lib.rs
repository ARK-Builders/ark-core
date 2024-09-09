
// uniffi::setup_scaffolding!() Must be called in the lib.rs file of the crate
mod router;

#[macro_export]
macro_rules! define_rpc {
    ($($name:ident),*) => {
        use once_cell::sync::Lazy;
        use router::Router;
        
        pub static ROUTER: Lazy<Router> = Lazy::new(|| {
            let mut router = Router::new(); 
            $(
                router.add(stringify!($name), $name);
            )*
            router
        });

        #[uniffi::export]
        pub fn call(path: String, data: Vec<String>) -> String {
            &ROUTER.call(path, data);
        }
    };
}