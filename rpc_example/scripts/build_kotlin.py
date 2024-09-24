import os
import platform
import subprocess
import shutil
import sys
import time


# Constants
KOTLINC = "kotlinc.bat"
CLASSPATH = os.getenv('CLASSPATH', './rpc_example/kotlin/vendor/jna.jar;./rpc_example/kotlin/vendor/kotlinx-coroutines.jar')  # Fetch CLASSPATH from the environment
LIB_NAME = "rpc_example"
TARGET_DIR = "./target/release"
KOTLIN_OUT_DIR = "./rpc_example/kotlin"

def run_command(command, print_only=False):
    """Run a command in the shell."""
    print(f"[COMMAND] {' '.join(command)}")
    if print_only:
        return

    return subprocess.run(command, shell=True)

def get_lib_extension():
    """Determine the correct library extension based on the operating system."""
    current_os = platform.system().lower()
    if current_os == "darwin":
        return "dylib"
    elif current_os == "linux":
        return "so"
    elif current_os == "windows":
        return "dll"
    else:
        sys.exit("Unknown OS. Supported OS: mac, linux, windows.")

def build_library():
    """Build the Rust library using cargo."""
    print("[INFO] Building library...")

    result = run_command(["cargo", "build", "-p", "rpc_example", "--release"])
    if result.returncode != 0:
        sys.exit("Failed to build library")

def generate_binding(lib_extension):
    """Generate Kotlin bindings using UniFFI."""
    print("[INFO] Generating Kotlin binding...")
    kotlin_lib_path = os.path.join(KOTLIN_OUT_DIR, LIB_NAME)
    if os.path.exists(kotlin_lib_path):
        shutil.rmtree(kotlin_lib_path)

    run_command(["cargo", "run", "-p", "rpc_example", "--features=uniffi/cli", "--bin", "uniffi-bindgen", "generate", 
         "--library", f"{TARGET_DIR}/{LIB_NAME}.{lib_extension}", 
         "--language", "kotlin", "--out-dir", KOTLIN_OUT_DIR])
    

def copy_cdylib(lib_extension):
    """Copy the built cdylib to the Kotlin output directory."""
    print("[INFO] Copying cdylib to output directory...")
    src = f"{TARGET_DIR}/{LIB_NAME}.{lib_extension}"
    dest = f"{KOTLIN_OUT_DIR}/{LIB_NAME}.{lib_extension}"

    print(f"[COMMAND] cp {src} {dest}")
    shutil.copy(src, dest)

def build_jar():
    """Compile Kotlin files into a JAR."""
    print("[INFO] Building Kotlin JAR...")
    jar_file = f"{KOTLIN_OUT_DIR}/rpc_example.jar"
    if os.path.exists(jar_file):
        os.remove(jar_file)

    run_command(
        [KOTLINC, "-Werror", "-d", jar_file, f"{KOTLIN_OUT_DIR}/uniffi/rpc_example/rpc_example.kt","-classpath",f'"{CLASSPATH}"'], True)

def run_tests():
    """Run Kotlin script tests."""
    print("[INFO] Executing tests...")
    test_names = ["blocking"]
    for test_name in test_names:
        print(f"[INFO] Running {test_name}_test.kts ...")
        run_command(
            [KOTLINC, "-Werror", "-J-ea", "-classpath", 
             f'"{CLASSPATH};{KOTLIN_OUT_DIR}/rpc_example.jar;{KOTLIN_OUT_DIR}"', 
             "-script", f"./rpc_example/tests/{test_name}_test.kts"],
            True
        )

def dependencies():
    vendor_folder = f"{KOTLIN_OUT_DIR}/vendor"

    if not os.path.exists(vendor_folder):
        os.makedirs(vendor_folder)

    # download jna.jar if it doesn't exist

    
    jna_url = "https://repo1.maven.org/maven2/net/java/dev/jna/jna/5.14.0/jna-5.14.0.jar"
    jna_file = f"{vendor_folder}/jna.jar"
    if os.path.exists(jna_file):
        print(f"[INFO] JNA already exists at {jna_file}")
    else:
        run_command(["curl", "-L", jna_url, "-o", jna_file])

    # download kotlinx-coroutines.jar
    kotlinx_coroutines_url = "https://repo1.maven.org/maven2/org/jetbrains/kotlinx/kotlinx-coroutines-core-jvm/1.6.4/kotlinx-coroutines-core-jvm-1.6.4.jar"
    kotlinx_coroutines_file = f"{vendor_folder}/kotlinx-coroutines.jar"
    if os.path.exists(kotlinx_coroutines_file):
        print(f"[INFO] kotlinx-coroutines already exists at {kotlinx_coroutines_file}")
    else:
        run_command(["curl", "-L", kotlinx_coroutines_url, "-o", kotlinx_coroutines_file])


def main():
    lib_extension = get_lib_extension()
    
    # Build library
    build_library()

    # Generate Kotlin bindings
    generate_binding(lib_extension)

    # Copy cdylib
    copy_cdylib(lib_extension)

    # Get 3rd party dependencies
    dependencies();

    # Build the Kotlin jar
    build_jar()

    # # Execute Kotlin tests
    run_tests()

if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as e:
        sys.exit(f"[ERROR] Command failed: {e}")
    except Exception as e:
        sys.exit(f"[ERROR] An error occurred: {e}")
