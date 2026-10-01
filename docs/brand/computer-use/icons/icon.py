from PIL import Image, ImageDraw
import math
INK=(29,26,23,255); PAPER=(236,230,218,255); PEN=(244,166,176,255)
def make(size, star=True):
    S=1024; im=Image.new("RGBA",(S,S),(0,0,0,0)); d=ImageDraw.Draw(im)
    m=S*0.03; d.rounded_rectangle([m,m,S-m,S-m], radius=S*0.22, fill=INK)
    # pointer arrow, classic shape, units of u
    u=S/26 if star else S/22; ox,oy=(S*0.25,S*0.17) if star else (S*0.26,S*0.15)
    pts=[(0,0),(0,17),(4.2,13.2),(7.2,20),(10,18.8),(7.1,12.2),(12.4,12.2)]
    P=[(ox+x*u,oy+y*u) for x,y in pts]
    d.polygon(P, fill=PAPER)
    if star:
        cx,cy,r,w=S*0.70,S*0.70,S*0.13,int(S*0.055)
        for k in range(3):
            a=math.pi/2+k*math.pi/3
            x1,y1=cx+r*math.cos(a),cy+r*math.sin(a); x2,y2=cx-r*math.cos(a),cy-r*math.sin(a)
            d.line([(x1,y1),(x2,y2)], fill=PEN, width=w)
            for (x,y) in [(x1,y1),(x2,y2)]: d.ellipse([x-w/2,y-w/2,x+w/2,y+w/2], fill=PEN)
    return im.resize((size,size), Image.LANCZOS)
for s in (16,32,48,128): make(s, star=s>=48).save(f"{s}.png")
big=Image.new("RGBA",(560,180),(242,237,226,255))
x=10
for s in (16,32,48,128):
    big.alpha_composite(make(s, star=s>=48),(x,20)); x+=s+20
big.alpha_composite(make(128).resize((128,128)),(x,20))
big.save("preview.png")
