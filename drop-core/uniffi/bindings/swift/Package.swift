// swift-tools-version: 5.9
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "ArkDrop",
    platforms: [
        .iOS(.v13),
        .macOS(.v10_15)
    ],
    products: [
        .library(
            name: "ArkDrop",
            targets: ["ArkDrop"]
        ),
    ],
    targets: [
        .target(
            name: "ArkDrop",
            dependencies: ["arkdrop_uniffiFFI"],
            path: "Sources"
        ),
        .binaryTarget(
            name: "arkdrop_uniffiFFI",
            path: "arkdrop_uniffiFFI.xcframework"
        )
    ]
)
