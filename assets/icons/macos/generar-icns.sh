#!/bin/sh
# Regenera los iconos de la aplicación (lorca.icns, lorca.ico y png/) a partir de
# macos/Lorca.icon (formato de Icon Composer). Sus capas salen de dibujar-capas.swift.
#
# Desde macOS 26 los iconos .icns que no tienen la forma de Apple se muestran dentro de un
# cuadro claro. Este script compila Lorca.icon con actool (Xcode 26 o posterior), pide a
# macOS que lo dibuje a 1024 px (con la forma y la sombra del sistema) y crea un .icns con
# todos los tamaños. Uso: sh assets/icons/macos/generar-icns.sh
set -eu
cd "$(dirname "$0")"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Rutas absolutas: el proceso auxiliar de actool puede seguir vivo con otra carpeta actual.
xcrun actool "$PWD/Lorca.icon" --compile "$tmp" --app-icon Lorca --platform macosx \
    --target-device mac --minimum-deployment-target 11.0 \
    --output-partial-info-plist "$tmp/partial.plist" >/dev/null

# Una app mínima con el Assets.car basta para que macOS dibuje el icono (con un
# ejecutable: si no, macOS le pone encima la señal de «prohibido»).
app="$tmp/Icono.app"
mkdir -p "$app/Contents/Resources" "$app/Contents/MacOS"
cp "$tmp/Assets.car" "$app/Contents/Resources/"
printf '#!/bin/sh\n' > "$app/Contents/MacOS/icono"
chmod +x "$app/Contents/MacOS/icono"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleExecutable</key><string>icono</string>
<key>CFBundleIdentifier</key><string>io.github.gallegomanuel8.lorca.icono</string>
<key>CFBundleIconName</key><string>Lorca</string>
</dict></plist>
PLIST

swiftc -O "$PWD/render-icon.swift" -o "$tmp/render"
"$tmp/render" "$app" "$tmp/master.png"

set=$tmp/Lorca.iconset
mkdir "$set"
for size in 16 32 128 256 512; do
    sips -z $size $size "$tmp/master.png" --out "$set/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z $double $double "$tmp/master.png" --out "$set/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$set" -o ../lorca.icns
# PNG para Windows, Linux y el icono de la ventana, y el .ico de Windows. Se recortan al
# cuerpo del icono (824 de 1024 px, centrado): el margen y la sombra son cosa de macOS; en
# Windows los iconos llenan el lienzo y con margen el de Lorca sale más pequeño que los demás.
sips -c 824 824 "$tmp/master.png" --out "$tmp/body.png" >/dev/null
for size in 16 24 32 48 64 128 256 512 1024; do
    sips -z $size $size "$tmp/body.png" --out "../png/lorca-$size.png" >/dev/null
done
python3 crear-ico.py ../lorca.ico ../png/lorca-16.png ../png/lorca-24.png \
    ../png/lorca-32.png ../png/lorca-48.png ../png/lorca-64.png ../png/lorca-128.png \
    ../png/lorca-256.png
echo "Creados assets/icons/lorca.icns, lorca.ico y png/"
