// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "Llavero",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "llavero", targets: ["llavero"]),
    ],
    targets: [
        .target(name: "LlaveroCore"),
        .executableTarget(name: "llavero", dependencies: ["LlaveroCore"]),
        .testTarget(name: "LlaveroTests", dependencies: ["LlaveroCore", "llavero"]),
    ]
)
