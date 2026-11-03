// Guarda en PNG (1024 px) el icono que macOS dibuja para un archivo o una app.
// Uso: render <ruta absoluta> <salida.png>
import AppKit
let path = CommandLine.arguments[1]
let out = CommandLine.arguments[2]
let icon = NSWorkspace.shared.icon(forFile: path)
icon.size = NSSize(width: 1024, height: 1024)
var rect = NSRect(x: 0, y: 0, width: 1024, height: 1024)
guard let cg = icon.cgImage(forProposedRect: &rect, context: nil, hints: nil) else { exit(1) }
let rep = NSBitmapImageRep(cgImage: cg)
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
