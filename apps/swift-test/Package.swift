// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "SwiftTest",
    platforms: [.macOS(.v26)],
    products: [.executable(name: "SwiftTest", targets: ["SwiftTest"])],
    targets: [.executableTarget(name: "SwiftTest")]
)
