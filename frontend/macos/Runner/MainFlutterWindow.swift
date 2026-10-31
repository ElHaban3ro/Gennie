import Cocoa
import FlutterMacOS
import CoreText

class MainFlutterWindow: NSWindow {
  private var assignmentLogos: AssignmentLogoResolver?
  private var tooltipChannel: FlutterMethodChannel?
  private var tooltipPanel: NSPanel?
  private var systemMotion: GennieSystemMotion?


  private var tooltipFonts: [String: NSFont] = [:]
  private func tooltipFont(_ face: String, size: CGFloat) -> NSFont {
    let base: NSFont
    if let cached = tooltipFonts[face] {
      base = cached
    } else {
      let path = "App.framework/Resources/flutter_assets/assets/fonts/Inter-\(face).ttf"
      if let frameworks = Bundle.main.privateFrameworksURL,
         let data = try? Data(contentsOf: frameworks.appendingPathComponent(path)),
         let provider = CGDataProvider(data: data as CFData), let font = CGFont(provider) {
        base = CTFontCreateWithGraphicsFont(font, 12, nil, nil) as NSFont
      } else {
        base = NSFont.systemFont(ofSize: 12,
          weight: face == "SemiBold" ? .semibold : face == "Regular" ? .regular : .medium)
      }
      tooltipFonts[face] = base
    }
    return NSFont(descriptor: base.fontDescriptor, size: size) ?? base
  }

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
                         textScale: args["textScale"] as? Double ?? 1,
                         details: args["details"] as? [String: Any])
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
                           dark: Bool, highContrast: Bool, preferRight: Bool, textScale: Double,
                           details: [String: Any]?) {
    hideTooltip()
    guard isVisible, isKeyWindow else { return }
    let body = GennieTooltipBody(text: text, details: details,
      textScale: textScale, maxWidth: max(80, min(300, view.bounds.width - 56)),
      font: { [self] face, size in tooltipFont(face, size: size) })
    let size = body.frame.size
    // Flutter cuenta desde arriba; AppKit convierte puntos de vista a pantalla.
    let local = NSRect(x: anchor.minX,
                       y: view.isFlipped ? anchor.minY : view.bounds.height - anchor.maxY,
                       width: anchor.width, height: anchor.height)
    let target = convertToScreen(view.convert(local, to: nil))
    var x = preferRight ? target.maxX + 4 : target.midX - size.width / 2
    var y = preferRight ? target.midY - size.height / 2 : target.minY - size.height - 6
    let windowBounds = convertToScreen(view.convert(view.bounds, to: nil)).insetBy(dx: 12, dy: 12)
    let visible = screen.map { windowBounds.intersection($0.visibleFrame.insetBy(dx: 8, dy: 8)) }
      ?? windowBounds
    if !visible.isEmpty {
      if preferRight && x + size.width > visible.maxX { x = target.minX - size.width - 4 }
      if !preferRight && y < visible.minY { y = target.maxY + 6 }
      x = max(visible.minX, min(x, visible.maxX - size.width))
      y = max(visible.minY, min(y, visible.maxY - size.height))
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
    let content = NSView(frame: bounds)
    content.addSubview(body)
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
      content.addSubview(effect, positioned: .below, relativeTo: body)
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
    assignmentLogos = AssignmentLogoResolver(controller)
    configureTooltips(controller)
    systemMotion = GennieSystemMotion(controller)
    (NSApp.delegate as? AppDelegate)?.stopSystemMotion = { [weak self] in
      self?.systemMotion?.stop()
    }
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

// El embedder actual no transmite Reducir movimiento a MediaQuery en macOS.
// Solo observamos AppKit: no cambiamos preferencias ni persistimos el valor.
private final class GennieSystemMotion: NSObject, FlutterStreamHandler {
  private let channel: FlutterEventChannel
  private var observer: NSObjectProtocol?
  private var sink: FlutterEventSink?

  init(_ controller: FlutterViewController) {
    channel = FlutterEventChannel(name: "gennie/system-motion",
      binaryMessenger: controller.engine.binaryMessenger)
    super.init()
    channel.setStreamHandler(self)
  }

  func onListen(withArguments arguments: Any?, eventSink events: @escaping FlutterEventSink) -> FlutterError? {
    stop()
    sink = events
    observer = NSWorkspace.shared.notificationCenter.addObserver(
      forName: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification,
      object: nil, queue: .main) { [weak self] _ in self?.publish() }
    publish()
    return nil
  }

  func onCancel(withArguments arguments: Any?) -> FlutterError? {
    stop()
    return nil
  }

  private func publish() {
    sink?(NSWorkspace.shared.accessibilityDisplayShouldReduceMotion)
  }

  func stop() {
    if let observer { NSWorkspace.shared.notificationCenter.removeObserver(observer) }
    observer = nil
    sink = nil
  }

  deinit { stop() }
}

// El mismo contenido estructurado del tooltip Flutter, con márgenes medidos en puntos.
private final class GennieTooltipBody: NSView {
  override var isFlipped: Bool { true }

  init(text: String, details: [String: Any]?, textScale: CGFloat, maxWidth: CGFloat,
       font: (String, CGFloat) -> NSFont) {
    super.init(frame: .zero)
    let padding: CGFloat = details == nil ? 7 : 16
    let horizontal: CGFloat = details == nil ? 14 : 16
    var y = padding

    func label(_ text: String, width: CGFloat, face: String = "Regular",
               size: CGFloat = 12, secondary: Bool = false, maxLines: Int = 0) -> NSTextField {
      let field = NSTextField(wrappingLabelWithString: text)
      field.font = font(face, size * textScale)
      let color: NSColor = secondary ? .secondaryLabelColor
        : face == "Regular" ? .labelColor.withAlphaComponent(0.88) : .labelColor
      field.textColor = color
      let style = NSMutableParagraphStyle()
      style.lineSpacing = details == nil ? 0 : 3 * textScale
      field.attributedStringValue = NSAttributedString(string: text, attributes: [
        .font: font(face, size * textScale),
        .foregroundColor: color,
        .paragraphStyle: style,
      ])
      field.maximumNumberOfLines = maxLines
      field.preferredMaxLayoutWidth = width
      let measured = field.attributedStringValue.boundingRect(
        with: NSSize(width: max(1, width - 4), height: CGFloat.greatestFiniteMagnitude),
        options: [.usesLineFragmentOrigin, .usesFontLeading])
      let limit = maxLines > 0 ? CGFloat(maxLines) * (size * textScale * 1.5) : CGFloat.greatestFiniteMagnitude
      field.setFrameSize(NSSize(width: width, height: min(ceil(measured.height) + 2, limit)))
      return field
    }
    func append(_ field: NSTextField, gap: CGFloat = 0) {
      y += gap
      field.setFrameOrigin(NSPoint(x: horizontal, y: y))
      addSubview(field)
      y += field.frame.height
    }
    if let details {
      let title = details["title"] as? String ?? ""
      let status = details["status"] as? String ?? ""
      let memory = details["memory"] as? String ?? "RAM —"
      let badge = label("\(status)  ·  \(memory)", width: maxWidth, face: "Medium", size: 11)
      let badgeText = NSMutableAttributedString(attributedString: badge.attributedStringValue)
      let memoryRange = NSRange(location: status.utf16.count,
        length: badgeText.length - status.utf16.count)
      badgeText.addAttribute(.font, value: font("Regular", 11 * textScale), range: memoryRange)
      badge.attributedStringValue = badgeText
      let badgeWidth = ceil(badge.attributedStringValue.size().width) + 4
      badge.setFrameSize(NSSize(width: badgeWidth, height: badge.frame.height))
      let titleLabel = label(title, width: max(40, maxWidth - badgeWidth - 24), face: "SemiBold", size: 13)
      titleLabel.setFrameOrigin(NSPoint(x: horizontal, y: y))
      badge.setFrameOrigin(NSPoint(x: horizontal + maxWidth - badgeWidth, y: y + 1))
      addSubview(titleLabel)
      addSubview(badge)
      let raw = (details["statusColor"] as? NSNumber)?.uint32Value ?? 0xff858585
      let dot = NSView(frame: NSRect(x: badge.frame.minX - 12, y: y + 6 * textScale,
                                     width: 7, height: 7))
      dot.wantsLayer = true
      dot.layer?.cornerRadius = 3.5
      dot.layer?.backgroundColor = NSColor(srgbRed: CGFloat((raw >> 16) & 255) / 255,
        green: CGFloat((raw >> 8) & 255) / 255, blue: CGFloat(raw & 255) / 255, alpha: 1).cgColor
      addSubview(dot)
      y += max(titleLabel.frame.height, badge.frame.height)
      if let message = details["message"] as? String, !message.isEmpty {
        append(label(message, width: maxWidth), gap: 12)
      }
      if let footer = details["footer"] as? String, !footer.isEmpty {
        y += 12
        let line = NSView(frame: NSRect(x: horizontal, y: y, width: maxWidth, height: 1))
        line.wantsLayer = true
        line.layer?.backgroundColor = NSColor.separatorColor.cgColor
        addSubview(line)
        y += 1
        append(label(footer, width: maxWidth, size: 11, secondary: true), gap: 10)
      }
      setFrameSize(NSSize(width: maxWidth + horizontal * 2, height: y + padding))
    } else {
      let field = label(text, width: maxWidth, face: "Medium")
      let width = min(maxWidth, ceil(field.attributedStringValue.size().width) + 4)
      field.setFrameSize(NSSize(width: width, height: field.frame.height))
      append(field)
      setFrameSize(NSSize(width: width + horizontal * 2, height: y + padding))
    }
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) no disponible") }
}
