import Cocoa
@main struct Probe {
  static func main() throws {
    guard let notes = AssignmentLogoResolver.application("Notas de Apple") else { fatalError("Notas no encontrada") }
    guard let png = AssignmentLogoResolver.png(NSWorkspace.shared.icon(forFile: notes.path)),
          let rep = NSBitmapImageRep(data: png), rep.pixelsWide == 80, rep.pixelsHigh == 80 else { fatalError("PNG inválido") }
    try png.write(to: URL(fileURLWithPath: CommandLine.arguments[1] + "/apple-notes.png"))
    let base = URL(string: "https://example.com/")!
    precondition(AssignmentLogoResolver.iconURL("<link href='/assets/logo.png' rel='shortcut icon'>", base: base)?.absoluteString == "https://example.com/assets/logo.png")
    precondition(AssignmentLogoResolver.iconURL("<link rel='apple-touch-icon' href='https://cdn.example.com/logo.png'>", base: base)?.host == "cdn.example.com")
    precondition(AssignmentLogoResolver.iconURL("<link rel='icon' href='file:///etc/passwd'>", base: base) == nil)
    precondition(AssignmentLogoResolver.origin("https://user:secret@example.com/path?secret=1#x")?.absoluteString == "https://example.com/")
    precondition(AssignmentLogoResolver.origin("file:///Applications/Notes.app") == nil)
    if ProcessInfo.processInfo.environment["GENNIE_LOGO_WEB_SMOKE"] == "1" {
      let done = DispatchSemaphore(value: 0)
      AssignmentLogoResolver.website(URL(string: "https://github.com/")!) { data in
        guard let data, let rep = NSBitmapImageRep(data: data), rep.pixelsWide == 80 else {
          fatalError("No se pudo obtener un favicon PNG real")
        }
        try! data.write(to: URL(fileURLWithPath: CommandLine.arguments[1] + "/github.png"))
        print("Web: favicon real convertido a PNG 80×80.")
        done.signal()
      }
      precondition(done.wait(timeout: .now() + 20) == .success)
    }
    print("Notas de Apple: icono real PNG 80×80; fuentes web relativas/externas y esquemas comprobados.")
  }
}
