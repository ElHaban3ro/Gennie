from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
import numpy as np
import subprocess, json

ROOT=Path(__file__).resolve().parents[1]
SLUGS=['cat','dog','rabbit','bear','fox','owl','penguin','frog','axolotl','turtle','hamster','seal']
LABELS=['Gato','Perro','Conejo','Oso','Zorro','Búho','Pingüino','Rana','Ajolote','Tortuga','Hámster','Foca']
FONT=ImageFont.truetype('/System/Library/Fonts/Helvetica.ttc',18)

def bbox(mask):
    yy,xx=np.nonzero(mask)
    return [int(xx.min()),int(yy.min()),int(xx.max()+1),int(yy.max()+1)] if len(xx) else None

def review(slug):
    p=ROOT/f'{slug}.svg'
    if not p.exists(): return None
    render=ROOT/'review'/f'{slug}-render.png'
    subprocess.run(['rsvg-convert','-w','362','-h','362','-o',str(render),str(p)],check=True,capture_output=True)
    src=Image.open(ROOT/'reference'/f'{slug}.png').convert('RGB')
    arr=np.array(src,dtype=float)
    border=np.concatenate([arr[:10].reshape(-1,3),arr[-10:].reshape(-1,3),arr[:,:10].reshape(-1,3),arr[:,-10:].reshape(-1,3)])
    bg=np.median(border,axis=0)
    mask=np.linalg.norm(arr-bg,axis=2)>12
    if slug=='hamster': mask[:,320:]=False  # Excluye la aleta de la foca vecina.
    svg=Image.open(render).convert('RGBA')
    sm=np.array(svg)[:,:,3]>127
    union=(mask|sm).sum()
    metrics={'source_bbox':bbox(mask),'svg_bbox':bbox(sm),'silhouette_iou_estimate':round(float((mask&sm).sum()/union),4)}
    rendered=Image.new('RGBA',(362,362),tuple(int(c) for c in bg)+(255,))
    rendered.alpha_composite(svg)
    pair=Image.new('RGB',(724,402),'#FCFAF3')
    pair.paste(src.crop((0,0,320,362)) if slug=='hamster' else src,(0,40));pair.paste(rendered.convert('RGB'),(362,40))
    d=ImageDraw.Draw(pair);d.text((12,10),f'{LABELS[SLUGS.index(slug)]} · Original',fill='#40392F',font=FONT);d.text((374,10),'SVG',fill='#40392F',font=FONT)
    pair.save(ROOT/'review'/f'{slug}-comparison.png')
    return pair,metrics

if __name__=='__main__':
    all_metrics={};out=Image.new('RGB',(1448,2412),'#FCFAF3')
    for i,slug in enumerate(SLUGS):
        result=review(slug)
        if result:
            pair,metrics=result;all_metrics[slug]=metrics
            out.paste(pair,((i%2)*724,(i//2)*402))
    out.save(ROOT/'comparison.png')
    (ROOT/'review'/'metrics.json').write_text(json.dumps(all_metrics,indent=2))
    print(json.dumps(all_metrics,indent=2))
