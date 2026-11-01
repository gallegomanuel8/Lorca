#!/usr/bin/env python3
"""Genera el icono de Lorca (assets/icons/*.png) sin dependencias externas.

Luna creciente plateada sobre verde nocturno («Verde que te quiero verde») y tres
líneas de texto. Las formas se dibujan con funciones de distancia con signo, así que
cada tamaño se calcula directamente y con bordes suaves.
"""

import math
import os
import struct
import zlib

SIZES = [16, 32, 128, 256, 512, 1024]
# Tamaños que se meten en icon.ico (Windows); el formato admite PNG dentro.
ICO_SIZES = [16, 32, 256]
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "icons")


def rounded_rect(px, py, cx, cy, hw, hh, r):
    qx = abs(px - cx) - hw + r
    qy = abs(py - cy) - hh + r
    outside = math.hypot(max(qx, 0.0), max(qy, 0.0))
    return outside + min(max(qx, qy), 0.0) - r


def circle(px, py, cx, cy, r):
    return math.hypot(px - cx, py - cy) - r


def mix(a, b, t):
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


def over(dst, color, alpha):
    """Compone `color` con opacidad `alpha` sobre `dst` (RGBA, 0..1)."""
    r, g, b, a = dst
    out_a = alpha + a * (1 - alpha)
    if out_a <= 0:
        return (0.0, 0.0, 0.0, 0.0)
    mixed = [(color[i] * alpha + (r, g, b)[i] * a * (1 - alpha)) / out_a for i in range(3)]
    return (mixed[0], mixed[1], mixed[2], out_a)


TOP = (0x1f / 255, 0x7a / 255, 0x5c / 255)
BOTTOM = (0x0c / 255, 0x2e / 255, 0x27 / 255)
MOON = (0xee / 255, 0xf4 / 255, 0xf0 / 255)
LINES = (0x8f / 255, 0xd9 / 255, 0xb6 / 255)


def pixel(x, y, size):
    # Coordenadas normalizadas (0..1) del centro del píxel y ancho de un píxel.
    u = (x + 0.5) / size
    v = (y + 0.5) / size
    px = 1.0 / size

    def coverage(distance):
        return min(max(0.5 - distance / px, 0.0), 1.0)

    color = (0.0, 0.0, 0.0, 0.0)
    # Fondo: cuadrado redondeado con margen, como los iconos de macOS.
    body = rounded_rect(u, v, 0.5, 0.5, 0.41, 0.41, 0.19)
    color = over(color, mix(TOP, BOTTOM, v), coverage(body))
    # Luna creciente: un círculo menos otro desplazado.
    outer = circle(u, v, 0.60, 0.38, 0.19)
    inner = circle(u, v, 0.68, 0.32, 0.165)
    moon = max(outer, -inner, body)
    color = over(color, MOON, coverage(moon))
    # Tres líneas de texto.
    for i, length in enumerate([0.30, 0.42, 0.22]):
        cy = 0.62 + i * 0.075
        hw = length / 2
        line = rounded_rect(u, v, 0.22 + hw, cy, hw, 0.019, 0.019)
        color = over(color, LINES, coverage(max(line, body)) * 0.95)
    return color


def png(size):
    rows = []
    for y in range(size):
        row = bytearray([0])
        for x in range(size):
            r, g, b, a = pixel(x, y, size)
            row.extend(int(round(c * 255)) for c in (r, g, b, a))
        rows.append(bytes(row))
    raw = b"".join(rows)

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def ico(images):
    """Archivo .ico con las imágenes PNG dadas ({tamaño: bytes})."""
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries = b""
    data = b""
    for size, image in images.items():
        dim = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(image), offset + len(data))
        data += image
    return header + entries + data


def main():
    os.makedirs(OUT, exist_ok=True)
    images = {}
    for size in SIZES:
        images[size] = png(size)
        # En .icns el de 1024 es el de 512 para pantallas Retina.
        name = "512x512@2x" if size == 1024 else f"{size}x{size}"
        path = os.path.join(OUT, f"{name}.png")
        with open(path, "wb") as file:
            file.write(images[size])
        print(path)
    path = os.path.join(OUT, "icon.ico")
    with open(path, "wb") as file:
        file.write(ico({size: images[size] for size in ICO_SIZES}))
    print(path)


if __name__ == "__main__":
    main()
