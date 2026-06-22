// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "GlyphRayMacHost",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "GlyphRayMacHost", targets: ["GlyphRayMacHost"])
    ],
    targets: [
        .executableTarget(
            name: "GlyphRayMacHost",
            path: "Sources/GlyphRayMacHost"
        ),
        .testTarget(
            name: "GlyphRayMacHostTests",
            dependencies: ["GlyphRayMacHost"],
            path: "Tests/GlyphRayMacHostTests"
        )
    ]
)
