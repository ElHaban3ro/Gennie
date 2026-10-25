// Rasteriza el icono vectorial existente con AppKit para el paquete macOS.
import AppKit
import Foundation

let arguments = CommandLine.arguments
if arguments.count != 3 { fatalError("Uso: render_app_icon.swift fuente.svg salida.png") }
let data = try Data(contentsOf: URL(fileURLWithPath: arguments[1]))
guard let image = NSImage(data: data), let tiff = image.tiffRepresentation,
      let bitmap = NSBitmapImageRep(data: tiff),
      let png = bitmap.representation(using: .png, properties: [:]) else {
    fatalError("No se pudo renderizar el icono de Gennie")
}
try png.write(to: URL(fileURLWithPath: arguments[2]))
