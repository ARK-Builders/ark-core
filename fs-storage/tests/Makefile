CARGO_CMD = cargo
RUST_BUILD_FLAGS = --release --features jni-bindings
JAR_FILE := junit.jar
URL := https://repo1.maven.org/maven2/org/junit/platform/junit-platform-console-standalone/1.9.3/junit-platform-console-standalone-1.9.3.jar

.PHONY: test
test: build-rust download-test-lib java-test

.PHONY: build-rust
build-rust:
	(cd .. && $(CARGO_CMD) build $(RUST_BUILD_FLAGS))

.PHONY: download-test-lib
download-test-lib:
	if [ ! -f $(JAR_FILE) ]; then \
		wget -O $(JAR_FILE) $(URL); \
	else \
		echo "$(JAR_FILE) already exists."; \
	fi

.PHONY: java-test
java-test:
	(cd ../../target/debug && export LD_LIBRARY_PATH=$$PWD && cd ../../fs-storage/tests && echo $$PWD && javac -d out FileStorage.java && javac -d out -cp out:$(JAR_FILE) FileStorageTest.java && java -jar $(JAR_FILE) --class-path out --scan-class-path)
