// Dibuja las capas del icono de la aplicación (1024 px, fondo transparente) para
// Lorca.icon: una luna llena roja (la luna redonda de Bodas de sangre, la «luna roja» de
// «Canción de jinete») y, delante, cuatro renglones de código sangrados que la cruzan como
// nubes. Colores de Mariana Pineda: tinta negra y rojo sobre marfil (el degradado de
// icon.json).
//
// Uso, desde esta carpeta: swift dibujar-capas.swift && sh generar-icns.sh

import AppKit

let size: CGFloat = 1024
let ink = NSColor(srgbRed: 0x1d / 255, green: 0x1d / 255, blue: 0x1d / 255, alpha: 1)
let red = NSColor(srgbRed: 0xb3 / 255, green: 0x26 / 255, blue: 0x2e / 255, alpha: 1)

/// Dibuja una capa con el origen arriba a la izquierda y la guarda en PNG.
func layer(_ path: String, _ draw: () -> Void) {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: Int(size), pixelsHigh: Int(size), bitsPerSample: 8,
        samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
        bytesPerRow: 0, bitsPerPixel: 0)!
    let context = NSGraphicsContext(bitmapImageRep: rep)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    context.cgContext.translateBy(x: 0, y: size)
    context.cgContext.scaleBy(x: 1, y: -1)
    draw()
    NSGraphicsContext.restoreGraphicsState()
    try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: path))
}

/// Trazo grueso con extremos y esquinas redondeados.
func stroke(_ points: [CGPoint], width: CGFloat, color: NSColor) {
    let path = NSBezierPath()
    path.move(to: points[0])
    for point in points.dropFirst() { path.line(to: point) }
    path.lineWidth = width
    path.lineCapStyle = .round
    path.lineJoinStyle = .round
    color.setStroke()
    path.stroke()
}

/// Luna llena: un disco.
func disc(center: CGPoint, radius: CGFloat, color: NSColor) {
    color.setFill()
    NSBezierPath(ovalIn: NSRect(x: center.x - radius, y: center.y - radius,
                                width: 2 * radius, height: 2 * radius)).fill()
}

// Los renglones van en la capa de delante (el primer grupo de icon.json).
layer("Lorca.icon/Assets/renglones.png") {
    let rows: [(y: CGFloat, from: CGFloat, to: CGFloat)] = [
        (429, 235, 505), (533, 315, 685), (637, 315, 585), (741, 235, 455),
    ]
    for row in rows {
        stroke([CGPoint(x: row.from, y: row.y), CGPoint(x: row.to, y: row.y)],
               width: 62, color: ink)
    }
}
layer("Lorca.icon/Assets/luna.png") {
    disc(center: CGPoint(x: 585, y: 487), radius: 236, color: red)
}
