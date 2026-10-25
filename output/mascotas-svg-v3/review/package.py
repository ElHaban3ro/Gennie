from pathlib import Path
import xml.etree.ElementTree as ET
import json,subprocess,tempfile,zipfile,shutil
from PIL import Image
ROOT=Path(__file__).resolve().parents[1]
NS='{http://www.w3.org/2000/svg}'
ET.register_namespace('','http://www.w3.org/2000/svg')
labels={'cat':'Gato','dog':'Perro','rabbit':'Conejo','bear':'Oso','fox':'Zorro','owl':'Búho','penguin':'Pingüino','frog':'Rana','axolotl':'Ajolote','turtle':'Tortuga','hamster':'Hámster','seal':'Foca'}
preview=ET.Element(NS+'svg',{'width':'1448','height':'1086','viewBox':'0 0 1448 1086'})
ET.SubElement(preview,NS+'rect',{'width':'1448','height':'1086','fill':'#FCFAF3'})
allids=set()
with tempfile.TemporaryDirectory(prefix='mascots-v3-final-') as tmp:
 for i,(slug,label) in enumerate(labels.items()):
  p=ROOT/f'{slug}.svg';doc=ET.parse(p).getroot()
  assert doc.get('viewBox')=='0 0 362 362',slug
  ids=[e.get('id') for e in doc.iter() if e.get('id')]
  assert len(ids)==len(set(ids)),slug
  assert not (set(ids)&allids),slug
  allids.update(ids)
  for e in doc.iter():
   assert e.tag not in (NS+'image',NS+'script',NS+'foreignObject')
   for attr,val in e.attrib.items():
    if 'href' in attr: assert val.startswith('#')
  for size in (64,512):
   out=Path(tmp)/f'{slug}-{size}.png'
   subprocess.run(['rsvg-convert','-w',str(size),'-h',str(size),'-o',str(out),str(p)],check=True,capture_output=True)
   im=Image.open(out).convert('RGBA');assert im.getextrema()[3][0]==0 and im.getextrema()[3][1]==255
  doc.set('x',str((i%4)*362));doc.set('y',str((i//4)*362));doc.set('width','362');doc.set('height','362')
  preview.append(doc)
  t=ET.SubElement(preview,NS+'text',{'x':str((i%4)*362+181),'y':str((i//4)*362+340),'font-family':'sans-serif','font-size':'17','text-anchor':'middle','fill':'#5B554C'});t.text=label
ET.ElementTree(preview).write(ROOT/'preview.svg',encoding='unicode')
subprocess.run(['rsvg-convert','-o',str(ROOT/'preview.png'),str(ROOT/'preview.svg')],check=True)
(ROOT/'README.md').write_text('''# Mascotas minimalistas — revisión 3

Doce SVGs independientes, editables, con fondo transparente. Cada archivo tiene tamaño nominal 512 × 512 y viewBox 0 0 362 362. Los identificadores internos llevan el nombre del animal para evitar colisiones al insertar varias mascotas en un documento.

## Referencia y revisión

Esta revisión parte de la imagen original generada, no de las versiones 1 y 2. Tres subagentes GPT-6.1 Sol con esfuerzo Medium trabajaron por grupos de cuatro mascotas. Cada uno abrió sus referencias, trazó las formas, renderizó a la misma escala, comparó y realizó varias pasadas de ajuste. Se revisaron siluetas, patas/aletas, orejas/branquias, ojos, colores y manchas. La aleta izquierda de la foca conserva su posición pegada al cuerpo de la referencia. La punta clara de la cola del zorro llega al contorno exterior; la zona naranja está recortada para evitar bordes naranjas alrededor de la punta.

comparison.png muestra cada original junto a su SVG a la misma escala. preview.png y preview.svg muestran solo la colección vectorial. Los recortes de referencia y registros de iteración están disponibles en las carpetas reference y review del directorio de trabajo; el ZIP de entrega contiene los SVGs, las vistas de conjunto y este documento.

Hay pequeñas diferencias de curvas y de textura. El raster original tiene grano y variaciones que los rellenos vectoriales suaves no reproducen exactamente. La comparación de máscaras registra coincidencia de silueta aproximada entre 97,6 y 98,9 por ciento; es una medición geométrica orientativa, no un porcentaje de identidad visual. La posición de los ojos se contrastó con centros medidos en la imagen fuente.

## Archivos

cat.svg — gato; dog.svg — perro; rabbit.svg — conejo; bear.svg — oso; fox.svg — zorro; owl.svg — búho; penguin.svg — pingüino; frog.svg — rana; axolotl.svg — ajolote; turtle.svg — tortuga; hamster.svg — hámster; seal.svg — foca.

## Comprobaciones

XML válido, identificadores únicos, ausencia de imágenes rasterizadas o recursos externos en las mascotas, transparencia y renderizado de los doce archivos a 64 y 512 píxeles. Revisión visual de las comparaciones. Esta biblioteca es una entrega independiente; no está integrada en la aplicación Gennie.
''')
archive=ROOT.parent/'mascotas-svg-v3.zip'
with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED) as z:
 for filename in [*(s+'.svg' for s in labels),'README.md','preview.svg','preview.png','comparison.png']:
  z.write(ROOT/filename,Path('mascotas-svg-v3')/filename)
with zipfile.ZipFile(archive) as z:assert z.testzip() is None
shutil.copyfile(archive,ROOT.parent/'mascotas-svg.zip')
print('Verificados 12 SVGs a 64/512 px, transparencia e identificadores. ZIP v3 verificado.')
print(archive)
