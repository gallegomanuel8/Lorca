# Icono de documento de Lorca: una hoja con renglones y, abajo a la izquierda, el logo
# (luna llena roja cruzada por renglones de código).
from PIL import Image, ImageDraw, ImageFilter
K = 4; S = 1024 * K
INK = (0x1d, 0x1d, 0x1d, 255); RED = (0xb3, 0x26, 0x2e, 255)
GREY = (0xb4, 0xad, 0x9e, 255)                 # renglones del documento, apagados
TOP = (254, 252, 247); BOT = (240, 233, 218)   # papel marfil
EDGE = (0xc9, 0xbf, 0xa8, 255); FOLD = (0xe0, 0xd6, 0xc0, 255)
L, T, R, B = 192, 64, 832, 960; RAD = 60; F = 190   # hoja y doblez

def bar(d, y, a, b, w, color):
    r = w * K / 2
    d.rounded_rectangle([a*K - r, y*K - r, b*K + r, y*K + r], radius=r, fill=color)

def dibujar():
    # silueta de la hoja con la esquina superior derecha cortada
    m = Image.new('L', (S, S), 0); d = ImageDraw.Draw(m)
    d.rounded_rectangle([L*K, T*K, R*K, B*K], radius=RAD*K, fill=255)
    d.polygon([((R-F)*K, T*K), (R*K+K, T*K-K), (R*K+K, (T+F)*K)], fill=0)
    out = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    # sombra suave
    sh = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    sh.paste(Image.new('RGBA', (S, S), (0, 0, 0, 70)), (0, 10*K), m)
    out.alpha_composite(sh.filter(ImageFilter.GaussianBlur(14*K)))
    # borde y papel
    out.paste(Image.new('RGBA', (S, S), EDGE), (0, 0), m)
    g = Image.new('RGB', (1, S))
    for y in range(S):
        t = min(1, max(0, (y - T*K) / ((B - T) * K)))
        g.putpixel((0, y), tuple(round(a + (b - a) * t) for a, b in zip(TOP, BOT)))
    inner = m.filter(ImageFilter.MinFilter(2 * 5 * K + 1))      # borde de 5 px
    out.paste(g.resize((S, S)).convert('RGBA'), (0, 0), inner)
    d = ImageDraw.Draw(out)
    # doblez
    fm = Image.new('L', (S, S), 0); fd = ImageDraw.Draw(fm)
    fd.rounded_rectangle([(R-F)*K, T*K, (R+60)*K, (T+F)*K], radius=0, fill=0)
    fd.polygon([((R-F)*K, T*K), (R*K, (T+F)*K), ((R-F)*K, (T+F)*K)], fill=255)
    fd.rounded_rectangle([(R-F)*K, (T+F-40)*K, (R-F+40)*K, (T+F)*K], radius=0, fill=255)
    out.paste(Image.new('RGBA', (S, S), EDGE), (0, 0), fm)
    out.paste(Image.new('RGBA', (S, S), FOLD), (0, 0), fm.filter(ImageFilter.MinFilter(2 * 5 * K + 1)))
    # renglones del documento
    for y, a, b in [(330, 280, 560), (420, 350, 740), (510, 350, 660)]:
        bar(d, y, a, b, 44, GREY)
    # logo abajo a la izquierda (geometría del icono de la app, reducida)
    s = 0.50; cx, cy = 420, 760
    def p(x, y): return cx + (x - 512) * s, cy + (y - 512) * s
    mx, my = p(585, 487); r = 236 * s
    d.ellipse([(mx-r)*K, (my-r)*K, (mx+r)*K, (my+r)*K], fill=RED)
    for y, a, b in [(429, 235, 505), (533, 315, 685), (637, 315, 585), (741, 235, 455)]:
        (xa, yy), (xb, _) = p(a, y), p(b, y)
        bar(d, yy, xa, xb, 62 * s, INK)
    return out.resize((1024, 1024), Image.LANCZOS)

if __name__ == '__main__':
    icon = dibujar(); icon.save('png/lorca-documento-1024.png')
    for s in [16, 24, 32, 48, 64, 128, 256, 512]:
        im = icon.resize((s, s), Image.LANCZOS)
        if s <= 48: im = im.filter(ImageFilter.UnsharpMask(radius=1, percent=50, threshold=0))
        im.save(f'png/lorca-documento-{s}.png')
    icon.save('lorca-documento.ico', sizes=[(s, s) for s in [16, 24, 32, 48, 64, 128, 256]])
    icon.save('lorca-documento.icns')
    sheet = Image.new('RGBA', (1500, 330), (240, 240, 240, 255))
    sheet.paste(Image.new('RGBA', (750, 330), (52, 52, 56, 255)), (750, 0))
    for ox in (0, 750):
        x = ox + 16
        for s in [256, 128, 64, 32, 16]:
            t = Image.open(f'png/lorca-documento-{s}.png'); sheet.paste(t, (x, 20), t); x += s + 10
        for s, z in [(32, 4), (16, 8)]:
            t = Image.open(f'png/lorca-documento-{s}.png').resize((s*z, s*z), Image.NEAREST)
            sheet.paste(t, (x + (0 if s == 32 else 0), 180 if s == 16 else 30), t)
    sheet.convert('RGB').save('vista-previa.png')
