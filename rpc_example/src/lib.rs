use rpc::{router::Router, uniffi_rpc_server};

uniffi::setup_scaffolding!();

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

pub fn add_array(left: u64, array: Vec<u64>) -> u64 {
    let a: u64 = array.iter().sum();
    return a + left;
}

uniffi_rpc_server!(add, add_array);


