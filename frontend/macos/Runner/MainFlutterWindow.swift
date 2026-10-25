import Cocoa
import FlutterMacOS
import CoreText

class MainFlutterWindow: NSWindow {
  private var tooltipChannel: FlutterMethodChannel?
  private var tooltipPanel: NSPanel?

  private lazy var tooltipFont: NSFont = {
    let path = "App.framework/Resources/flutter_assets/assets/fonts/Inter-Medium.ttf"
    guard let frameworks = Bundle.main.privateFrameworksURL,
          let data = try? Data(contentsOf: frameworks.appendingPathComponent(path)),
          let provider = CGDataProvider(data: data as CFData),
          let font = CGFont(provider) else {
      return NSFont.systemFont(ofSize: 12, weight: .medium)
    }
    return CTFontCreateWithGraphicsFont(font, 12, nil, nil) as NSFont
  }()

  // Una ventana hija permite al compositor desenfocar también la ventana Flutter.
  private func configureTooltips(_ controller: FlutterViewController) {
    let channel = FlutterMethodChannel(
      name: "gennie/tooltip", binaryMessenger: controller.engine.binaryMessenger)
    tooltipChannel = channel
    channel.setMethodCallHandler { [weak self, weak controller] call, result in
      guard let self, let controller else { result(nil); return }
      switch call.method {
      case "hide":
        self.hideTooltip()
        result(nil)
      case "show":
        guard let args = call.arguments as? [String: Any],
              let label = args["label"] as? String,
              let x = args["x"] as? Double, let y = args["y"] as? Double,
              let width = args["width"] as? Double,
              let height = args["height"] as? Double else {
          result(FlutterError(code: "invalid_tooltip", message: "Tooltip inválido", details: nil))
          return
        }
        self.showTooltip(label, anchor: NSRect(x: x, y: y, width: width, height: height),
                         view: controller.view, dark: args["dark"] as? Bool ?? false,
                         highContrast: args["highContrast"] as? Bool ?? false,
                         preferRight: args["preferRight"] as? Bool ?? true,
                         textScale: args["textScale"] as? Double ?? 1)
        result(nil)
      default:
        result(FlutterMethodNotImplemented)
      }
    }
    for name in [NSWindow.didResignKeyNotification, NSWindow.willMiniaturizeNotification,
                 NSWindow.willCloseNotification, NSWindow.willMoveNotification,
                 NSWindow.didResizeNotification] {
      NotificationCenter.default.addObserver(self, selector: #selector(hideTooltip),
                                             name: name, object: self)
    }
  }

  @objc private func hideTooltip() {
    if let panel = tooltipPanel {
      removeChildWindow(panel)
      panel.orderOut(nil)
      tooltipPanel = nil
    }
  }

  private func showTooltip(_ text: String, anchor: NSRect, view: NSView,
                           dark: Bool, highContrast: Bool, preferRight: Bool, textScale: Double) {
    hideTooltip()
    guard isVisible, isKeyWindow else { return }
    let label = NSTextField(labelWithString: text)
    label.font = NSFont(descriptor: tooltipFont.fontDescriptor, size: 12 * textScale)
    label.textColor = .labelColor
    label.sizeToFit()
    let size = NSSize(width: ceil(label.frame.width) + 28, height: ceil(label.frame.height) + 14)
    // Flutter cuenta desde arriba; AppKit convierte puntos de vista a pantalla.
    let local = NSRect(x: anchor.minX,
                       y: view.isFlipped ? anchor.minY : view.bounds.height - anchor.maxY,
                       width: anchor.width, height: anchor.height)
    let target = convertToScreen(view.convert(local, to: nil))
    var x = preferRight ? target.maxX + 4 : target.midX - size.width / 2
    var y = preferRight ? target.midY - size.height / 2 : target.minY - size.height - 6
    if let visible = screen?.visibleFrame {
      if preferRight && x + size.width > visible.maxX { x = target.minX - size.width - 4 }
      if !preferRight && y < visible.minY { y = target.maxY + 6 }
      x = max(visible.minX + 8, min(x, visible.maxX - size.width - 8))
      y = max(visible.minY + 8, min(y, visible.maxY - size.height - 8))
    }
    let panel = NSPanel(contentRect: NSRect(x: x, y: y,
                                            width: size.width, height: size.height),
                        styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = false
    panel.ignoresMouseEvents = true
    panel.hidesOnDeactivate = true
    panel.isReleasedWhenClosed = false
    panel.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
    let bounds = NSRect(origin: .zero, size: size)
    label.setFrameOrigin(NSPoint(x: 14, y: (size.height - label.frame.height) / 2))
    let content = NSView(frame: bounds)
    content.addSubview(label)
    let opaque = highContrast || NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency
    if opaque {
      content.wantsLayer = true
      content.layer?.cornerRadius = 8
      content.layer?.backgroundColor = (dark ? NSColor(white: 0.15, alpha: 1)
                                             : NSColor(white: 0.96, alpha: 1)).cgColor
      panel.contentView = content
    } else {
      // El tooltip solo necesita desenfoque: el vidrio refractivo genera aristas.
      let effect = NSVisualEffectView(frame: bounds)
      effect.material = .hudWindow
      effect.blendingMode = .behindWindow
      effect.state = .active
      effect.wantsLayer = true
      effect.layer?.cornerRadius = 8
      effect.layer?.masksToBounds = true
      // El efecto completo elimina el fondo nítido; la etiqueta queda encima.
      content.addSubview(effect, positioned: .below, relativeTo: label)
      panel.contentView = content
    }
    addChildWindow(panel, ordered: .above)
    panel.orderFront(nil)
    tooltipPanel = panel
  }

  override func awakeFromNib() {
    let controller = FlutterViewController()
    let windowFrame = NSRect(origin: frame.origin, size: NSSize(width: 1100, height: 720))
    contentViewController = controller
    setFrame(windowFrame, display: true)
    contentMinSize = NSSize(width: 640, height: 420)
    title = "Gennie"
    titleVisibility = .hidden
    titlebarAppearsTransparent = true
    styleMask.insert(.fullSizeContentView)
    isOpaque = false
    backgroundColor = .clear
    controller.backgroundColor = .clear
    hasShadow = false
    if let content = contentView {
      let material: NSView
      if #available(macOS 26.0, *) {
        let glass = NSGlassEffectView(frame: content.bounds)
        glass.style = .regular
        material = glass
      } else {
        let effect = NSVisualEffectView(frame: content.bounds)
        effect.material = .underWindowBackground
        effect.blendingMode = .behindWindow
        effect.state = .active
        material = effect
      }
      material.autoresizingMask = [.width, .height]
      content.addSubview(material, positioned: .below, relativeTo: nil)
    }
    (NSApp.delegate as? AppDelegate)?.configureLifecycle(controller)
    configureTooltips(controller)
    RegisterGeneratedPlugins(registry: controller)
    super.awakeFromNib()
    positionWindowButtons()
  }
  override func layoutIfNeeded() {
    super.layoutIfNeeded()
    positionWindowButtons()
  }
  private func positionWindowButtons() {
    guard !styleMask.contains(.fullScreen),
          let titlebar = standardWindowButton(.closeButton)?.superview,
          let container = titlebar.superview,
          let frameView = container.superview else { return }
    // Compartir los 56 pt del encabezado Flutter también en el área interactiva.
    let headerHeight: CGFloat = 56
    container.frame = NSRect(x: container.frame.minX,
                              y: frameView.bounds.maxY - headerHeight,
                              width: container.frame.width, height: headerHeight)
    titlebar.frame = NSRect(x: titlebar.frame.minX, y: 0,
                             width: titlebar.frame.width, height: headerHeight)
    for (index, kind) in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton].enumerated() {
      if let button = standardWindowButton(kind) {
        var frame = button.frame
        frame.origin.x = 72 + CGFloat(index) * 22
        frame.origin.y = (headerHeight - frame.height) / 2
        button.setFrameOrigin(frame.origin)
      }
    }
  }
}
