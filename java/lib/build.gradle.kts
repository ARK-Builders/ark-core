plugins {
    // Apply the java-library plugin for API and implementation separation.
    `java-library`
}

repositories {
    // Use Maven Central for resolving dependencies.
    mavenCentral()
}

dependencies {
    // Use JUnit Jupiter for testing.
    testImplementation(libs.junit.jupiter)

    testRuntimeOnly("org.junit.platform:junit-platform-launcher")

    // This dependency is exported to consumers, that is to say found on their compile classpath.
    api(libs.commons.math3)

    // This dependency is used internally, and not exposed to consumers on their own compile classpath.
    implementation(libs.guava)
}

// Apply a specific Java toolchain to ease working on different environments.
java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(21)
    }
}

tasks.named<Test>("test") {
    // Use JUnit Platform for unit tests.
    useJUnitPlatform()
    // Set the JVM argument for the java.library.path (To import rust compiled library)
    val rustLibPath = projectDir.resolve("../../target/release").absolutePath
    jvmArgs = listOf("-Djava.library.path=$rustLibPath")
}

tasks.named<Javadoc>("javadoc") {
    options.encoding = "UTF-8"
    options.memberLevel = JavadocMemberLevel.PUBLIC
}
