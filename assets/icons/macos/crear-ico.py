"""Crea un .ico con imágenes PNG dentro (Windows Vista y posteriores las admiten).

Uso: python3 crear-ico.py salida.ico imagen1.png imagen2.png ...
"""

import struct
import sys


def png_size(data):
    # Cabecera PNG: la anchura y la altura están en el fragmento IHDR.
    return struct.unpack(">II", data[16:24])


def main():
    out, *paths = sys.argv[1:]
    images = [open(path, "rb").read() for path in paths]
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries = b""
    for data in images:
        width, height = png_size(data)
        # 0 significa 256 px.
        entries += struct.pack(
            "<BBBBHHII", width % 256, height % 256, 0, 0, 1, 32, len(data), offset
        )
        offset += len(data)
    with open(out, "wb") as f:
        f.write(header + entries + b"".join(images))


if __name__ == "__main__":
    main()
