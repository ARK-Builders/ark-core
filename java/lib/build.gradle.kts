plugins {
    `maven-publish` // Apply the maven-publish plugin before java-library for publishing to GitHub Packages.
    `java-library`  // Apply the java-library plugin for API and implementation separation.
}

group = "dev.arkbuilders.core"
version = "1.0-SNAPSHOT"

repositories {
    mavenCentral()  // Use Maven Central for resolving dependencies.
}

dependencies {
    testImplementation(libs.junit.jupiter) // Use JUnit Jupiter for testing.
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
    api(libs.commons.math3)     // This dependency is exported to consumers, that is to say found on their compile classpath.
    implementation(libs.guava)  // This dependency is used internally, and not exposed to consumers on their own compile classpath.
}

// Apply a specific Java toolchain to ease working on different environments.
java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(21)
    }
}

tasks.named<Test>("test") {
    val rustLibPath = projectDir.resolve("../../target/release").absolutePath  // Set the JVM argument for the java.library.path (To import rust compiled library)
    useJUnitPlatform()                                                         // Use JUnit Platform for unit tests.
    jvmArgs = listOf("-Djava.library.path=$rustLibPath")
}

tasks.named<Javadoc>("javadoc") {
    options.encoding = "UTF-8"
    options.memberLevel = JavadocMemberLevel.PUBLIC
}

publishing {
    // Define a Maven publication for the 'maven' repository
    publications {
        create<MavenPublication>("Maven") {
            from(components["java"])
            pom {
                name.set("fs_storage")
                description.set("File system storage bindings for writing key value pairs to disk.")
            }
        }
    }
    repositories {
        maven {
            name = "GitHubPackages"
            url = uri("https://maven.pkg.github.com/ARK-Builders/ark-core")
            credentials {
                username = System.getenv("GITHUB_ACTOR")
                password = System.getenv("GITHUB_TOKEN")
            }
        }
    }
}
