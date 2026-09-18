// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "MetaprobeSwift",
    platforms: [.iOS(.v13)],
    products: [
        .library(name: "MetaprobeSwift", targets: ["MetaprobeSwift"]),
    ],
    targets: [
        .target(
            name: "MetaprobeSwift",
            dependencies: ["MetaprobeFFI"],
            path: "Sources/MetaprobeSwift"
        ),
        .binaryTarget(
            name: "MetaprobeFFI",
            path: "Metaprobe.xcframework"
        ),
    ]
)
