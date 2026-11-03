// Dibuja las capas del icono de la aplicación (1024 px, fondo transparente) para
// Lorca.icon: una luna creciente (la del «Romancero gitano») y líneas de texto con un
// cursor. El fondo verde («Verde que te quiero verde») es el degradado de icon.json.
//
// Uso, desde esta carpeta: swift dibujar-capas.swift && sh generar-icns.sh

import AppKit
import CoreGraphics

let S: CGFloat = 1024
let ivory = CGColor(srgbRed: 0.95, green: 0.92, blue: 0.85, alpha: 1)
let gold = CGColor(srgbRed: 0.85, green: 0.66, blue: 0.29, alpha: 1)

func image(_ draw: (CGContext) -> Void) -> CGImage {
    let ctx = CGContext(data: nil, width: Int(S), height: Int(S), bitsPerComponent: 8, bytesPerRow: 0,
                        space: CGColorSpace(name: CGColorSpace.sRGB)!,
                        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    // Origen arriba a la izquierda.
    ctx.translateBy(x: 0, y: S); ctx.scaleBy(x: 1, y: -1)
    draw(ctx)
    return ctx.makeImage()!
}

func save(_ img: CGImage, _ name: String) {
    let rep = NSBitmapImageRep(cgImage: img)
    try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: name))
}

func moon(_ ctx: CGContext) {
    // Luna creciente: círculo menos otro desplazado.
    ctx.beginTransparencyLayer(auxiliaryInfo: nil)
    ctx.setFillColor(ivory)
    ctx.fillEllipse(in: CGRect(x: 500, y: 130, width: 380, height: 380))
    ctx.setBlendMode(.clear)
    ctx.fillEllipse(in: CGRect(x: 610, y: 90, width: 330, height: 330))
    ctx.setBlendMode(.normal)
    ctx.endTransparencyLayer()
}

func lines(_ ctx: CGContext) {
    let h: CGFloat = 66
    let rows: [(CGFloat, CGFloat, CGFloat, CGColor)] = [
        (190, 450, 380, ivory),
        (290, 560, 440, gold),
        (290, 670, 300, ivory),
        (190, 780, 260, ivory),
    ]
    for (x, y, w, color) in rows {
        ctx.setFillColor(color)
        ctx.addPath(CGPath(roundedRect: CGRect(x: x, y: y, width: w, height: h), cornerWidth: h / 2, cornerHeight: h / 2, transform: nil))
        ctx.fillPath()
    }
    // Cursor de texto tras la última línea.
    ctx.setFillColor(gold)
    ctx.addPath(CGPath(roundedRect: CGRect(x: 488, y: 753, width: 38, height: 120), cornerWidth: 14, cornerHeight: 14, transform: nil))
    ctx.fillPath()
}

save(image(moon), "Lorca.icon/Assets/luna.png")
save(image(lines), "Lorca.icon/Assets/lineas.png")
