// Dibuja las capas del icono de la aplicación (1024 px, fondo transparente) para
// Lorca.icon: los signos `< >` de las etiquetas de código y, entre ellos, una luna
// creciente roja (la luna de Lorca: «Romance de la luna, luna», Bodas de sangre). Colores
// de Mariana Pineda: tinta negra y rojo sobre marfil (el degradado de icon.json).
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

/// Luna creciente: un círculo menos otro desplazado hacia arriba a la derecha.
func crescent(center: CGPoint, radius: CGFloat, color: NSColor) {
    let context = NSGraphicsContext.current!.cgContext
    context.beginTransparencyLayer(auxiliaryInfo: nil)
    color.setFill()
    context.fillEllipse(in: CGRect(x: center.x - radius, y: center.y - radius,
                                   width: 2 * radius, height: 2 * radius))
    context.setBlendMode(.clear)
    let cut = radius * 0.86
    context.fillEllipse(in: CGRect(x: center.x - cut + radius * 0.42, y: center.y - cut - radius * 0.22,
                                   width: 2 * cut, height: 2 * cut))
    context.setBlendMode(.normal)
    context.endTransparencyLayer()
}

layer("Lorca.icon/Assets/signos.png") {
    let width: CGFloat = 92
    stroke([CGPoint(x: 330, y: 330), CGPoint(x: 170, y: 512), CGPoint(x: 330, y: 694)],
           width: width, color: ink)
    stroke([CGPoint(x: 694, y: 330), CGPoint(x: 854, y: 512), CGPoint(x: 694, y: 694)],
           width: width, color: ink)
}
layer("Lorca.icon/Assets/luna.png") {
    crescent(center: CGPoint(x: 512, y: 512), radius: 168, color: red)
}
