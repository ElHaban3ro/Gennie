import AppKit
import CoreGraphics
import CoreImage
import ImageIO
import SwiftUI

private enum GlassMode: String, CaseIterable {
    case swiftUI = "SwiftUI glassEffect"
    case appKit = "AppKit NSGlassEffectView"
    case wallpaper = "Fondo local (experimental)"
}

private final class FaceModel: ObservableObject {
    @Published var mode: GlassMode = .swiftUI
    @Published var wallpaperPatch: NSImage?
    @Published var desktopVisible = true
}

private enum WallpaperMaterial {
    @MainActor
    static func isDesktopVisible(under panel: NSPanel) -> Bool {
        guard let primary = NSScreen.screens.first,
              let windows = CGWindowListCopyWindowInfo(
                [.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID
              ) as? [[String: Any]] else {
            return false
        }
        let face = CGRect(
            x: panel.frame.minX + 16,
            y: primary.frame.maxY - panel.frame.maxY + 16,
            width: 128,
            height: 128
        )
        let ownPID = Int(ProcessInfo.processInfo.processIdentifier)
        for window in windows {
            guard let layer = window[kCGWindowLayer as String] as? Int,
                  layer == 0,
                  let pid = window[kCGWindowOwnerPID as String] as? Int,
                  pid != ownPID,
                  let bounds = window[kCGWindowBounds as String] as? [String: Any],
                  let x = bounds["X"] as? CGFloat,
                  let y = bounds["Y"] as? CGFloat,
                  let width = bounds["Width"] as? CGFloat,
                  let height = bounds["Height"] as? CGFloat else { continue }
            if CGRect(x: x, y: y, width: width, height: height).intersects(face) {
                return false
            }
        }
        return true
    }

    @MainActor
    static func patch(for panel: NSPanel) -> NSImage? {
        guard let screen = panel.screen ?? NSScreen.main,
              let url = NSWorkspace.shared.desktopImageURL(for: screen),
              let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
            return nil
        }

        // El escritorio usa aspect fill centrado. AppKit mide desde abajo; CGImage desde arriba.
        let display = screen.frame
        let face = panel.frame.insetBy(dx: 16, dy: 16)
        let sample = face.insetBy(dx: -20, dy: -20)
        let imageWidth = CGFloat(image.width)
        let imageHeight = CGFloat(image.height)
        let visibleWidth = min(imageWidth, imageHeight * display.width / display.height)
        let visibleHeight = min(imageHeight, imageWidth * display.height / display.width)
        let visibleX = (imageWidth - visibleWidth) / 2
        let visibleY = (imageHeight - visibleHeight) / 2
        let sourceRect = CGRect(
            x: visibleX + (sample.minX - display.minX) / display.width * visibleWidth,
            y: visibleY + (display.maxY - sample.maxY) / display.height * visibleHeight,
            width: sample.width / display.width * visibleWidth,
            height: sample.height / display.height * visibleHeight
        ).integral
        guard let crop = image.cropping(to: sourceRect),
              let bitmap = CGContext(
                data: nil, width: 168, height: 168, bitsPerComponent: 8,
                bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
              ) else {
            return nil
        }
        bitmap.interpolationQuality = .high
        bitmap.draw(crop, in: CGRect(x: 0, y: 0, width: 168, height: 168))
        guard let pixels = bitmap.makeImage() else { return nil }
        let context = CIContext()
        let input = CIImage(cgImage: pixels)
        let blur = CIFilter(name: "CIGaussianBlur", parameters: [
            kCIInputImageKey: input, kCIInputRadiusKey: 10
        ])?.outputImage ?? input
        let lens = CIFilter(name: "CITorusLensDistortion", parameters: [
            kCIInputImageKey: blur,
            kCIInputCenterKey: CIVector(x: 84, y: 84),
            kCIInputRadiusKey: 64,
            kCIInputWidthKey: 16,
            kCIInputRefractionKey: 1.7
        ])?.outputImage ?? blur
        let faceRect = CGRect(x: 20, y: 20, width: 128, height: 128)
        guard let output = context.createCGImage(lens, from: faceRect) else { return nil }
        return NSImage(cgImage: output, size: NSSize(width: 128, height: 128))
    }
}

private struct NativeGlass: NSViewRepresentable {
    func makeNSView(context: Context) -> NSGlassEffectView {
        let glass = NSGlassEffectView(frame: NSRect(x: 0, y: 0, width: 128, height: 128))
        glass.style = .regular
        glass.cornerRadius = 64
        glass.tintColor = nil
        glass.contentView = NSView(frame: glass.bounds)
        return glass
    }

    func updateNSView(_ glass: NSGlassEffectView, context: Context) {}
}

private struct Mouth: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        path.move(to: CGPoint(x: rect.minX + 53, y: rect.minY + 75))
        path.addQuadCurve(
            to: CGPoint(x: rect.minX + 75, y: rect.minY + 75),
            control: CGPoint(x: rect.minX + 64, y: rect.minY + 92)
        )
        return path
    }
}

private struct FaceFeatures: View {
    var body: some View {
        ZStack(alignment: .topLeading) {
            Ellipse()
                .fill(Color(red: 242 / 255, green: 242 / 255, blue: 242 / 255))
                .frame(width: 11, height: 18)
                .offset(x: 36, y: 44)
            Ellipse()
                .fill(Color(red: 242 / 255, green: 242 / 255, blue: 242 / 255))
                .frame(width: 11, height: 18)
                .offset(x: 82, y: 44)
            Mouth()
                .stroke(
                    Color(red: 242 / 255, green: 242 / 255, blue: 242 / 255),
                    style: StrokeStyle(lineWidth: 2, lineCap: .round)
                )
        }
        .frame(width: 128, height: 128)
        .accessibilityLabel("Cara de prueba de Gennie")
    }
}

private struct FaceView: View {
    @ObservedObject var model: FaceModel

    var body: some View {
        ZStack {
            if model.mode == .swiftUI {
                Color.clear
                    .frame(width: 128, height: 128)
                    .glassEffect(.regular, in: Circle())
            } else if model.mode == .appKit {
                NativeGlass()
                    .frame(width: 128, height: 128)
            } else if model.desktopVisible, let patch = model.wallpaperPatch {
                Image(nsImage: patch)
                    .resizable()
                    .frame(width: 128, height: 128)
                    .clipShape(Circle())
            } else {
                NativeGlass()
                    .frame(width: 128, height: 128)
            }
            FaceFeatures()
            Circle()
                .strokeBorder(Color.white.opacity(0.65), lineWidth: 1)
                .frame(width: 128, height: 128)
        }
        .frame(width: 128, height: 128)
        .frame(width: 160, height: 160)
    }
}

private final class DragHostingView: NSHostingView<FaceView> {
    var didDrag: (() -> Void)?

    override func mouseDown(with event: NSEvent) {
        window?.performDrag(with: event)
        didDrag?()
    }
}

@MainActor
private final class AppDelegate: NSObject, NSApplicationDelegate {
    private let model = FaceModel()
    private var panel: NSPanel?
    private var statusItem: NSStatusItem?
    private var swiftUIItem: NSMenuItem?
    private var appKitItem: NSMenuItem?
    private var wallpaperItem: NSMenuItem?
    private var visibilityTimer: Timer?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)

        let frame = NSRect(x: 0, y: 0, width: 160, height: 160)
        let panel = NSPanel(
            contentRect: frame,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.title = "Swift Test"
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.hidesOnDeactivate = false
        let content = DragHostingView(rootView: FaceView(model: model))
        content.didDrag = { [weak self] in self?.updateWallpaperPatch() }
        panel.contentView = content

        if let screen = NSScreen.main {
            let visible = screen.visibleFrame
            panel.setFrameOrigin(NSPoint(x: visible.maxX - 220, y: visible.maxY - 640))
        }
        if let index = CommandLine.arguments.firstIndex(of: "--origin"),
           CommandLine.arguments.count > index + 2,
           let x = Double(CommandLine.arguments[index + 1]),
           let y = Double(CommandLine.arguments[index + 2]) {
            panel.setFrameOrigin(NSPoint(x: x, y: y))
        }
        panel.orderFrontRegardless()
        self.panel = panel
        if CommandLine.arguments.contains("--wallpaper") {
            model.mode = .wallpaper
        }
        updateWallpaperPatch()
        refreshDesktopVisibility()
        visibilityTimer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in self?.refreshDesktopVisibility() }
        }

        let menu = NSMenu()
        let swiftUIItem = NSMenuItem(
            title: GlassMode.swiftUI.rawValue,
            action: #selector(showSwiftUI),
            keyEquivalent: ""
        )
        let appKitItem = NSMenuItem(
            title: GlassMode.appKit.rawValue,
            action: #selector(showAppKit),
            keyEquivalent: ""
        )
        let wallpaperItem = NSMenuItem(
            title: GlassMode.wallpaper.rawValue,
            action: #selector(showWallpaper),
            keyEquivalent: ""
        )
        swiftUIItem.target = self
        appKitItem.target = self
        wallpaperItem.target = self
        menu.addItem(swiftUIItem)
        menu.addItem(appKitItem)
        menu.addItem(wallpaperItem)
        menu.addItem(.separator())
        let quitItem = NSMenuItem(title: "Salir de Swift Test", action: #selector(quit), keyEquivalent: "")
        quitItem.target = self
        menu.addItem(quitItem)

        let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        statusItem.button?.title = "Swift Test"
        statusItem.menu = menu
        self.statusItem = statusItem
        self.swiftUIItem = swiftUIItem
        self.appKitItem = appKitItem
        self.wallpaperItem = wallpaperItem
        updateMenu()
    }

    @objc private func showSwiftUI() {
        model.mode = .swiftUI
        updateMenu()
    }

    @objc private func showAppKit() {
        model.mode = .appKit
        updateMenu()
    }

    @objc private func showWallpaper() {
        updateWallpaperPatch()
        model.mode = .wallpaper
        updateMenu()
    }

    @MainActor @objc private func quit() {
        visibilityTimer?.invalidate()
        NSApp.terminate(nil)
    }

    private func updateMenu() {
        swiftUIItem?.state = model.mode == .swiftUI ? .on : .off
        appKitItem?.state = model.mode == .appKit ? .on : .off
        wallpaperItem?.state = model.mode == .wallpaper ? .on : .off
    }

    private func updateWallpaperPatch() {
        guard let panel else { return }
        model.wallpaperPatch = WallpaperMaterial.patch(for: panel)
        refreshDesktopVisibility()
    }

    private func refreshDesktopVisibility() {
        guard let panel else { return }
        let visible = WallpaperMaterial.isDesktopVisible(under: panel)
        if visible != model.desktopVisible {
            model.desktopVisible = visible
            if visible { model.wallpaperPatch = WallpaperMaterial.patch(for: panel) }
        }
    }
}

let app = NSApplication.shared
private let delegate = AppDelegate()
app.delegate = delegate
app.run()
