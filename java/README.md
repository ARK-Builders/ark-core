# `ark-java`

This subdirectory contains the Java bindings for `ark` core Rust code. These bindings use [JNI](https://en.wikipedia.org/wiki/Java_Native_Interface) (Java Native Interface).

The process involves generating dynamic libraries from Rust code, which are then loaded into the Java runtime. This is achieved by compiling the Rust code into shared libraries (`.so` on Linux, `.dylib` on macOS, and `.dll` on Windows), which are then linked and loaded at runtime using the [`System.loadLibrary`](https://docs.oracle.com/javase/8/docs/api/java/lang/Runtime.html#loadLibrary-java.lang.String-) method in Java. This method loads the specified native library, making its functions available for use in a Java environment.

We use Gradle for dependency management.

## Building the Bindings

To build the bindings and set up the environment, follow these steps:

### Prerequisites

- Ensure you have Gradle installed. You can install it from [Gradle](https://gradle.org/install/_)
- Ensure you have the Rust toolchain installed. You can install it from [rustup](https://rustup.rs/).
- Ensure you have a JDK installed. You can download it from [AdoptOpenJDK](https://adoptopenjdk.net/).
- Ensure you have Android NDK version `28.0.12674087` installed. You can install it from [NDK](https://github.com/android/ndk/releases/tag/r28-rc1)

### Steps

1. **Add Android Architecture Targets:** Run the following command to add support for different Android OS architectures:

```sh
rustup add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

This will make possible to compile the Rust code for different Android OS architectures.

2. **Build the Java Project:** To compile build the project, run:

```sh
./gradlew build
```

This will compile the Java code and generate the JAR file in the `lib/build/libs` directory. This will also run the unit tests.

## Cleaning the Build

To clean the build, run:

```sh
./gradlew clean
```

This will clean the build and remove the generated files.
