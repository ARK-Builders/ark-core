import uniffi.*
import kotlinx.coroutines.*
import kotlin.system.*

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

    val time = measureTimeMillis {
        val one = uniffi.rpc_example.call("add_with_wait", listOf("10", "20"))
        val two = uniffi.rpc_example.call("add_with_wait", listOf("10", "20"))
        println("The answer is ${one + two}")
    }
    println("Completed sync in $time ms")


    val time2 = measureTimeMillis {
        println("Starting async test")
        val one = async { uniffi.rpc_example.callAsync("add_with_wait", listOf("10", "20")) }
        println("First async call")
        val two = async { uniffi.rpc_example.callAsync("add_with_wait", listOf("10", "20")) }
        println("The answer is ${one.await() + two.await()}")
    }
    println("Completed async in $time2 ms")


    println("Blocking Test Passed")

}