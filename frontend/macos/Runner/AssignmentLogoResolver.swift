import Cocoa
import CryptoKit
import FlutterMacOS

// Solo recursos visuales: no lee datos de las apps ni abre sesiones de usuario.
final class AssignmentLogoResolver {
  private let channel: FlutterMethodChannel
  private let queue: OperationQueue = {
    let queue = OperationQueue()
    queue.maxConcurrentOperationCount = 2
    queue.qualityOfService = .utility
    return queue
  }()

  init(_ controller: FlutterViewController) {
    channel = FlutterMethodChannel(name: "gennie/assignment-logo",
      binaryMessenger: controller.engine.binaryMessenger)
    channel.setMethodCallHandler { [weak self] call, result in
      guard call.method == "resolve" else { result(FlutterMethodNotImplemented); return }
      guard let self, let args = call.arguments as? [String: String],
            let name = args["name"], name.count <= 200 else { result(nil); return }
      let kind = args["kind"] ?? ""
      let url = args["url"] ?? ""
      // NSWorkspace pertenece al hilo principal; la descarga y PNG quedan fuera.
      let app = kind == "desktop" ? Self.application(name) : nil
      self.queue.addOperation {
        let key = "\(kind)\n\(name)\n\(url)"
        let path = Self.cachePath(key)
        if let path, let attrs = try? FileManager.default.attributesOfItem(atPath: path.path),
           let date = attrs[.modificationDate] as? Date,
           Date().timeIntervalSince(date) < 7 * 86400,
           let data = try? Data(contentsOf: path), data.count <= 1_048_576 {
          DispatchQueue.main.async { result(FlutterStandardTypedData(bytes: data)) }
          return
        }
        if let app {
          DispatchQueue.main.async {
            let data = Self.png(NSWorkspace.shared.icon(forFile: app.path))
            self.deliver(data, path: path, result: result)
          }
        } else if kind == "web", let origin = Self.origin(url) {
          let finished = DispatchSemaphore(value: 0)
          Self.website(origin) { data in
            self.deliver(data, path: path, result: result)
            finished.signal()
          }
          finished.wait()
        } else {
          DispatchQueue.main.async { result(nil) }
        }
      }
    }
  }

  private func deliver(_ data: Data?, path: URL?, result: @escaping FlutterResult) {
    queue.addOperation {
      if let data, let path { try? data.write(to: path, options: .atomic) }
      DispatchQueue.main.async { result(data.map { FlutterStandardTypedData(bytes: $0) }) }
    }
  }

  static func normalized(_ name: String) -> String {
    name.folding(options: [.diacriticInsensitive, .caseInsensitive], locale: Locale(identifier: "en_US"))
      .filter { $0.isLetter || $0.isNumber }
  }

  static func application(_ name: String) -> URL? {
    let aliases = ["applenotes": "com.apple.Notes", "notasdeapple": "com.apple.Notes",
      "notes": "com.apple.Notes", "notas": "com.apple.Notes",
      "applereminders": "com.apple.reminders", "recordatorios": "com.apple.reminders",
      "reminders": "com.apple.reminders", "safari": "com.apple.Safari",
      "applemail": "com.apple.mail", "mail": "com.apple.mail"]
    let key = normalized(name)
    if let id = aliases[key], let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: id) {
      return url
    }
    // Comparación exacta contra nombres reales y localizados; nunca interpreta rutas.
    let roots = ["/Applications", "/System/Applications", "/System/Applications/Utilities",
      FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Applications").path]
    for root in roots {
      let urls = (try? FileManager.default.contentsOfDirectory(at: URL(fileURLWithPath: root),
        includingPropertiesForKeys: nil, options: [.skipsHiddenFiles])) ?? []
      for url in urls where url.pathExtension == "app" {
        let bundle = Bundle(url: url)
        let names = [url.deletingPathExtension().lastPathComponent,
          bundle?.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String ?? "",
          bundle?.object(forInfoDictionaryKey: "CFBundleName") as? String ?? ""]
        if names.contains(where: { normalized($0) == key }) { return url }
      }
    }
    return nil
  }

  static func origin(_ raw: String) -> URL? {
    guard var parts = URLComponents(string: raw),
          ["https", "http"].contains(parts.scheme?.lowercased() ?? ""),
          let host = parts.host, !host.isEmpty else { return nil }
    parts.user = nil; parts.password = nil; parts.path = "/"
    parts.query = nil; parts.fragment = nil
    return parts.url
  }

  static func png(_ image: NSImage) -> Data? {
    guard image.isValid, let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil,
      pixelsWide: 80, pixelsHigh: 80, bitsPerSample: 8, samplesPerPixel: 4,
      hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
      bytesPerRow: 0, bitsPerPixel: 0), let context = NSGraphicsContext(bitmapImageRep: bitmap)
    else { return nil }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    let scale = min(80 / max(1, image.size.width), 80 / max(1, image.size.height))
    let size = NSSize(width: image.size.width * scale, height: image.size.height * scale)
    image.draw(in: NSRect(x: (80 - size.width) / 2, y: (80 - size.height) / 2,
      width: size.width, height: size.height), from: .zero, operation: .copy, fraction: 1)
    NSGraphicsContext.restoreGraphicsState()
    return bitmap.representation(using: .png, properties: [:])
  }

  private static func cachePath(_ key: String) -> URL? {
    guard let root = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first else { return nil }
    let dir = root.appendingPathComponent("dev.fer.gennie/assignment-logos", isDirectory: true)
    do { try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true) }
    catch { return nil }
    let hash = SHA256.hash(data: Data(key.utf8)).map { String(format: "%02x", $0) }.joined()
    return dir.appendingPathComponent(hash + ".png")
  }

  static func website(_ origin: URL, completion: @escaping (Data?) -> Void) {
    // Favicon del propio origen, sin servicios de terceros ni cookies de navegador.
    LimitedLogoDownload.fetch(origin.appendingPathComponent("favicon.ico")) { data in
      if let data, let image = NSImage(data: data), let png = Self.png(image) {
        completion(png)
      } else {
        // Muchas webs solo anuncian PNG/SVG mediante <link rel="icon">.
        LimitedLogoDownload.fetch(origin) { html in
          guard let html, let text = String(data: html, encoding: .utf8),
                let icon = Self.iconURL(text, base: origin) else { completion(nil); return }
          LimitedLogoDownload.fetch(icon) { data in
            completion(data.flatMap { NSImage(data: $0) }.flatMap(Self.png))
          }
        }
      }
    }
  }

  static func iconURL(_ html: String, base: URL) -> URL? {
    let tags = try? NSRegularExpression(pattern: "<link\\b[^>]*>", options: .caseInsensitive)
    let attribute = try? NSRegularExpression(pattern: "([a-zA-Z-]+)\\s*=\\s*[\"']([^\"']*)[\"']")
    let ns = html as NSString
    for tag in tags?.matches(in: html, range: NSRange(location: 0, length: ns.length)) ?? [] {
      let text = ns.substring(with: tag.range) as NSString
      var attrs = [String: String]()
      for match in attribute?.matches(in: text as String, range: NSRange(location: 0, length: text.length)) ?? [] {
        attrs[text.substring(with: match.range(at: 1)).lowercased()] = text.substring(with: match.range(at: 2))
      }
      let rel = (attrs["rel"] ?? "").lowercased().split(whereSeparator: { $0.isWhitespace })
      if rel.contains("icon") || rel.contains("apple-touch-icon"), let href = attrs["href"],
         let url = URL(string: href.replacingOccurrences(of: "&amp;", with: "&"), relativeTo: base)?.absoluteURL,
         ["http", "https"].contains(url.scheme?.lowercased() ?? ""), url.user == nil, url.password == nil {
        return url
      }
    }
    return nil
  }
}

// Máximo 1 MiB y 5 s por recurso, incluidas respuestas sin Content-Length.
private final class LimitedLogoDownload: NSObject, URLSessionDataDelegate {
  private var data = Data()
  private let completion: (Data?) -> Void
  private var valid = false
  private init(_ completion: @escaping (Data?) -> Void) { self.completion = completion }
  static func fetch(_ url: URL, completion: @escaping (Data?) -> Void) {
    let delegate = LimitedLogoDownload(completion)
    let config = URLSessionConfiguration.ephemeral
    config.timeoutIntervalForRequest = 5
    config.timeoutIntervalForResource = 5
    config.httpShouldSetCookies = false
    config.httpCookieStorage = nil
    let session = URLSession(configuration: config, delegate: delegate, delegateQueue: nil)
    var request = URLRequest(url: url)
    request.setValue("Gennie/1.0", forHTTPHeaderField: "User-Agent")
    session.dataTask(with: request).resume()
  }
  func urlSession(_ session: URLSession, dataTask: URLSessionDataTask,
                  didReceive response: URLResponse, completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
    valid = (response as? HTTPURLResponse).map { (200...299).contains($0.statusCode) } == true
      && response.expectedContentLength <= 1_048_576
    completionHandler(valid ? .allow : .cancel)
  }
  func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive chunk: Data) {
    if data.count + chunk.count > 1_048_576 { valid = false; dataTask.cancel() }
    else { data.append(chunk) }
  }
  func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
    completion(valid && error == nil ? data : nil)
    session.finishTasksAndInvalidate()
  }
}
