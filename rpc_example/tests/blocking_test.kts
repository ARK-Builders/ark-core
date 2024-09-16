import uniffi.*

kotlinx.coroutines.runBlocking {
    // function factorial does not exist
    var factorial = uniffi.rpc_example.call("factorial", listOf("10"))
    assert(factorial == "{\"result\": null, \"error\": \"NOT_FOUND: Unknown function\", \"is_success\": false}")

    // testing single argument function
    var add = uniffi.rpc_example.call("add", listOf("10", "20"))
    assert(add == "{\"result\":30,\"error\":null,\"is_success\":true}")

    // testing multiple argument function
    var add_array = uniffi.rpc_example.call("add_array", listOf("10", "[1,2,3,4,5,6]"))
    assert(add_array == "{\"result\":31,\"error\":null,\"is_success\":true}")
}