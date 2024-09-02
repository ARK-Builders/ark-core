
// #[derive(serde::Deserialize)]
// struct RPCRequest {
//     func_name: String,
//     args: Vec<String>,
// }

// #[derive(serde::Serialize)]
// struct RPCResponse {
//     data: String,
//     error: String,
//     is_successful: bool,
// }

// impl TryInto<String> for RPCResponse {
//     type Error = serde_json::Error;

//     fn try_into(self) -> Result<String> {
//         serde_json::to_string(&self)
//     }
// }

// impl TryFrom<String> for RPCRequest {
//     type Error = serde_json::Error;

//     fn try_from(s: String) -> Result<Self> {
//         serde_json::from_str(&s)
//     }
// }

// fn success(data: impl Serialize) -> Result<RPCResponse> {
//     let response = RPCResponse {
//         data: serde_json::to_string(&data)?,
//         error: "".to_string(),
//         is_successful: true,
//     };
//     Ok(response)
// }

// fn error(error: String) -> RPCResponse {
//     RPCResponse {
//         data: "".to_string(),
//         error: error,
//         is_successful: false,
//     }
// }

// struct RpcRouter {
//     routes: HashMap<String, Box<(dyn Handler + 'static)>>,
// }


// trait Handler {
//     fn call(&self, args: Vec<String>) -> String;
// }