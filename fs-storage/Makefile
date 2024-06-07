CARGO_CMD = cargo
JAR_FILE := junit.jar
URL := https://repo1.maven.org/maven2/org/junit/platform/junit-platform-console-standalone/1.9.3/junit-platform-console-standalone-1.9.3.jar

.PHONY: test-linux
test-linux: build-rust download-test-lib java-test-linux

.PHONY: test-mac
test-mac: build-rust download-test-lib java-test-mac

.PHONY: test-windows
test-windows: build-rust download-test-lib java-test-windows

.PHONY: build-rust
build-rust:
	$(CARGO_CMD) build

.PHONY: download-test-lib
download-test-lib: 
	cd tests && \
	if [ ! -f "$(JAR_FILE)" ]; then \
		curl -o "$(JAR_FILE)" "$(URL)"; \
	else \
		echo "$(JAR_FILE) already exists."; \
	fi

.PHONY: java-test-linux
java-test-linux:
	(cd ../target/debug && \
	export LD_LIBRARY_PATH=$$PWD && \
	cd ../../fs-storage/tests && \
	javac -d out FileStorage.java && \
	javac -d out -cp out:$(JAR_FILE) FileStorageTest.java && \
	RUST_BACKTRACE=1 java -jar $(JAR_FILE) --class-path out --scan-class-path)

.PHONY: java-test-mac
java-test-mac:
	(cd ../target/debug && \
	export DYLD_LIBRARY_PATH=$$PWD && \
	cd ../../fs-storage/tests && \
	javac -d out FileStorage.java && \
	javac -d out -cp out:$(JAR_FILE) FileStorageTest.java && \
	RUST_BACKTRACE=1 java -jar $(JAR_FILE) --class-path out --scan-class-path)

.PHONY: java-test-windows
java-test-windows:
	(cd ../target/debug && \
	export LIBRARY_PATH="$$PWD" && \
	cd ../../fs-storage/tests && \
	javac -d out FileStorage.java && \
	javac -d out -cp "out;$(JAR_FILE)" FileStorageTest.java && \
	RUST_BACKTRACE=1 java -Djava.library.path=$$LIBRARY_PATH -jar $(JAR_FILE) --class-path out --scan-class-path)