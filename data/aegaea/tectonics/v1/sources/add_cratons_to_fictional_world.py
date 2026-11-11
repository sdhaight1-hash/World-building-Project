"""Add invented cratonic basement domains to the established inverted/mirrored fictional world.
Cratons are NOT plate labels. Each belongs to an existing fictional plate in this PRESENT snapshot.
The core masks, rock ages, and shield/platform distinctions are hypothetical design inputs,
not a reconstructed geological history or actual Earth craton data.
"""
from pathlib import Path
import math, json
import numpy as np
import cv2
from scipy.ndimage import gaussian_filter, distance_transform_edt
from PIL import Image, ImageDraw, ImageFont, ImageColor

OUT=Path('/mnt/data'); w,h=1672,941
src=np.asarray(Image.open(OUT/'mirrored_upside_down_physical_world_map.png').convert('RGB'))
R,G,B=src[:,:,0].astype(np.int16),src[:,:,1].astype(np.int16),src[:,:,2].astype(np.int16)
sea=((B-R>72)&(B-G>35)&(B>176)).astype(np.uint8)
sea=cv2.morphologyEx(sea,cv2.MORPH_CLOSE,np.ones((2,2),np.uint8))
land=(1-sea).astype(bool)

# EXACT present-day plate seed geometry from existing fictional generator.
plates=[('NIVALE',.51,.012),('KORU',.183,.226),('LYRAN',.026,.535),('VEYRA',.271,.565),('SEREVAN',.268,.823),('BRINEVEIL',.448,.852),('ANKAR',.440,.179),('MERROW',.523,.413),('THALASS',.647,.726),('CAERIN',.676,.176),('RUSKEL',.678,.438),('ORREN',.876,.221),('HALDEN',.821,.554),('VALLIN',.812,.839),('AUREOLE',.985,.565),('DRAEM',.533,.979),('SUNDRAL',.267,.385)]
y=np.linspace(0,1,h,dtype=np.float32)[:,None]
x=np.linspace(0,1,w,dtype=np.float32)[None,:]
xa=(x+.019*np.sin(14*y+8*x)+.010*np.sin(26*y-17*x))%1.0
ya=y+.019*np.sin(13*x+6*y)+.013*np.sin(33*x-13*y)
dist=np.stack([ (np.minimum(abs(xa-sx),1-abs(xa-sx))*1.75)**2+((ya-sy)*1.02)**2 for _,sx,sy in plates])
plateid=dist.argmin(axis=0).astype(np.uint8)
cap=.079+.012*np.sin(x*11.9)+.007*np.sin(x*34.4)
plateid=np.where(y<cap,0,plateid)

# Defined in normalized coordinates on the established fixed-coastline map:
# (name,cx,cy,radius_x,radius_y,rotation_deg,basement_age_Ga,dominant_exposure)
cratons=[
('KAVRA', .199,.245,.063,.060,  9,3.20,'platform'), # isolated western-island block
('ORVEL', .113,.547,.060,.075,-25,3.34,'shield'),
('MAREN', .262,.690,.100,.075, -7,2.85,'platform'),
('ILTHEN',.377,.717,.073,.065, 23,2.74,'platform'),
('KEVAR', .254,.800,.065,.044, 15,3.05,'shield'),
('SYRIN', .527,.364,.059,.099,  7,3.45,'shield'),
('EIDEN', .528,.527,.060,.060,-23,2.94,'platform'),
('NALOR', .540,.645,.038,.039,-38,2.58,'shield'),
('TAVEN', .930,.204,.050,.064,-15,3.18,'platform'),
('SELDAR',.799,.542,.066,.053,  7,2.82,'shield'),
('HERIN', .836,.729,.065,.063, 25,3.38,'platform'),
('VORUN', .906,.807,.040,.045, 17,2.69,'platform'),
('FARON', .627,.818,.047,.057,  8,3.08,'shield'),
]
# Build ancient coherent interiors. Restrict to present-day plate for present-day coherent blocks.
# Contained in the assigned plate; avoids placing undisturbed old shields across active boundaries.
labels=np.zeros((h,w),np.uint8)
shield_mask=np.zeros((h,w),np.uint8)
platform_mask=np.zeros((h,w),np.uint8)
counts=[]
for idx,(name,cx,cy,rx,ry,ang,age,exposure) in enumerate(cratons,1):
    qx=np.argmin(abs(np.arange(w)/w-cx));qy=np.argmin(abs(np.arange(h)/h-cy))
    # Move seed toward an existing land pixel if an input center is inadvertently off-land.
    if not land[qy,qx]:
        yy,xx=np.where(land & ((x-cx)**2 + (y-cy)**2 < .03**2))
        if len(xx):
            qx=int(xx[np.argmin((xx/w-cx)**2+(yy/h-cy)**2)]);qy=int(yy[np.argmin((xx/w-cx)**2+(yy/h-cy)**2)])
    pid=plateid[qy,qx]
    th=np.deg2rad(ang)
    X=(x-cx)*np.cos(th)+(y-cy)*np.sin(th)
    Y=-(x-cx)*np.sin(th)+(y-cy)*np.cos(th)
    rho=np.sqrt((X/rx)**2+(Y/ry)**2)
    # An independent smooth irregularity field: no Earth basement geometry imported.
    irregular=.085*np.sin(15*x+18*y+idx*2)+.063*np.sin(41*x-24*y+idx)+.03*np.cos(84*x+52*y+idx)
    core=(rho < (1+irregular)) & land & (plateid==pid) & (labels==0)
    # Only ancient stable interiors, not the whole plate; no hard protected-distance buffer at current boundary.
    labels[core]=idx
    # Shield vs platform are SURFACE exposure facies of the same basement domain.
    q=(X/rx)**2+(Y/ry)**2
    outcrops=(q < (.38 if exposure=='shield' else .14)+.12*np.sin(x*55+y*40+idx))
    shield_mask[core & outcrops]=idx
    platform_mask[core & ~outcrops]=idx
    area=int(core.sum()); counts.append({'name':name,'age_ga':age,'plate':plates[pid][0],'pixels':area, 'surface':exposure})

# Paleozoic/Proterozoic collisional mobile zones follow margins, but ARE NOT active plate boundaries.
# A low-strength transitional belt around old cores, only on land.
base=(labels>0).astype(np.uint8)
edge_dist=distance_transform_edt(1-base)
mobile=((edge_dist>0)&(edge_dist<10)&land).astype(np.uint8)
# Additional pre-existing orogens/suture trace are interpretive, not required to coincide with active plates.

# Draw on true synthetic elevation image; underlay is physical terrain, NOT old Earth terrain.
preview_base=Image.open(OUT/'reimagined_geography_physical_preview.png').convert('RGBA')
# Work in the computed grid and upscale; labels drawn full 8K then resize for preview.
terrain=Image.open(OUT/'reimagined_geography_physical_8k.png').convert('RGBA')
TW,TH=terrain.size
# Age colors: purple for old, rust for younger; display shields as bright solid patches and platforms transparent hatch-like tint.
age_categories=[('Older basement  3.5–3.2 Ga','#cf79a4'),('Younger basement  3.2–2.5 Ga','#f2b574'),('Old mobile belts','#5baeb4')]
layer=np.zeros((h,w,4),np.uint8)
# Black suture belt faded into existing elevation map.
layer[mobile>0]=[49,153,159,112]
for i,(_,cx,cy,rx,ry,ang,age,_) in enumerate(cratons,1):
    old=age>=3.2
    c=(200,103,165) if old else (242,175,102)
    sel=(labels==i)
    layer[sel]=(*c,108)
    # Ancient shield zones crystalline basement: stronger tint.
    layer[shield_mask==i]=(*c,167)
    # Platform zones sediment covered: slightly pale.
    layer[platform_mask==i]=(*(np.clip(np.array(c)*.78+np.array([220,216,185])*.22,0,255).astype('uint8')),117)

overlay=Image.fromarray(layer,'RGBA').resize((TW,TH),Image.Resampling.BILINEAR)
full=Image.alpha_composite(terrain,overlay)
# Boundary outlines drawn at computational grid and scaled; subtle white edges within continent.
cont=np.zeros((h,w),np.uint8)
for i in range(1,len(cratons)+1):
    a=(labels==i).astype(np.uint8)
    if np.count_nonzero(a)<15: continue
    boundary=cv2.morphologyEx(a,cv2.MORPH_GRADIENT,np.ones((3,3),np.uint8))
    cont=np.maximum(cont,boundary*130)
outline=Image.fromarray(cont,'L').resize((TW,TH),Image.Resampling.NEAREST)
white=Image.new('RGBA',(TW,TH),(248,240,219,0));white.putalpha(outline)
full=Image.alpha_composite(full,white)
D=ImageDraw.Draw(full,'RGBA')
font='/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf';bold='/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf'
TITLE=ImageFont.truetype(bold,83);SUB=ImageFont.truetype(font,37);H=ImageFont.truetype(bold,44);L=ImageFont.truetype(bold,38);S=ImageFont.truetype(font,31)
D.rounded_rectangle((90,70,3230,410),radius=36,fill=(6,21,39,225),outline=(211,231,237,151),width=3)
D.text((158,123),'ANCIENT CRATONS & SHIELDS',font=TITLE,fill=(251,250,246,255))
D.text((164,246),'13 invented basement cores · fixed mirrored coastlines · distinct from present plates',font=SUB,fill=(221,233,237,255))
D.text((164,316),'Basement ages are hypothetical · shield / platform exposure is schematic',font=SUB,fill=(200,219,230,255))
# Legend inset in the upper ocean: preserve all craton labels on the continents.
D.rounded_rectangle((3340,70,5890,460),radius=31,fill=(6,21,39,228),outline=(211,231,237,138),width=3)
D.text((3410,114),'CRATONIC BASEMENT',font=H,fill=(252,252,249,255))
for i,(label,col) in enumerate(age_categories):
    yy=200+i*78
    D.rounded_rectangle((3414,yy,3538,yy+48),radius=8,fill=ImageColor.getrgb(col)+(215,))
    D.text((3585,yy+4),label,font=SUB,fill=(246,246,244,255))
D.rounded_rectangle((6030,70,7565,460),radius=31,fill=(6,21,39,228),outline=(211,231,237,138),width=3)
D.text((6090,111),'SURFACE EXPOSURE',font=H,fill=(252,252,249,255))
D.text((6090,190),'Solid: crystalline shield',font=SUB,fill=(227,239,245,255))
D.text((6090,250),'Pale: sedimentary platform',font=SUB,fill=(227,239,245,255))
D.text((6090,310),'Cyan fringe: ancient mobile belt',font=S,fill=(204,226,236,255))
D.text((6090,364),'Not present-day plate outlines',font=S,fill=(204,226,236,255))
# labels: use actual centroid of core, avoiding accidental ocean positions.
for i, rec in enumerate(counts,1):
    ys,xs=np.where(labels==i)
    if len(xs)<45: continue
    cx=float(xs.mean())/w;cy=float(ys.mean())/h
    xx=int(cx*TW); yy=int(cy*TH)
    text=rec['name']; age=rec['age_ga']
    D.text((xx,yy),text,font=L,anchor='mm',fill=(250,250,249,255),stroke_width=5,stroke_fill=(13,22,31,255))
    D.text((xx,yy+47),f'{age:.2f} Ga',font=S,anchor='mm',fill=(248,245,224,255),stroke_width=3,stroke_fill=(13,22,31,225))

f=OUT/'fictional_cratons_geology_8k.png';full.convert('RGB').save(f,compress_level=2)
fprev=OUT/'fictional_cratons_geology_preview.png';full.resize((1920,1080),Image.Resampling.LANCZOS).convert('RGB').save(fprev)
Image.fromarray(labels,'L').save(OUT/'fictional_craton_id_mask_1672x941.png')
Image.fromarray((shield_mask>0).astype('uint8')*255,'L').save(OUT/'fictional_shields_mask_1672x941.png')
Image.fromarray((mobile>0).astype('uint8')*255,'L').save(OUT/'fictional_paleomobile_belts_mask_1672x941.png')
summary={'craton_count':len(cratons),'craton_cores':counts,'mask_value': '0=noncratonic;1..13 correspond to craton_cores order','cratons_are_tectonic_plates':False,'tectonic_model':'17 invented present-day plates','created_from':'mirrored upside down land mask and procedural geologic domain seeds','limitations':['A hypothetical surface basement map, not a dynamic reconstruction of multi-Ga plate history','Domains artificially clipped to single present-day plates; historical rifting and rifted fragments not explicitly modeled','Shields, sedimentary platforms, and paleo-mobile zones are designed, not inferred from stratigraphic evidence','Existing terrain and elevation were not modified by this overlay']}
(OUT/'fictional_cratons_metadata.json').write_text(json.dumps(summary,indent=2))
print('Land pixels:',land.sum(),'assigned craton pixels:',(labels>0).sum(),'fraction of land',round((labels>0).sum()/land.sum(),3))
for rec in counts: print(rec)
for out in [f,fprev,OUT/'fictional_cratons_metadata.json',OUT/'fictional_craton_id_mask_1672x941.png']:
    print(out.name, round(out.stat().st_size/1e6,2),'MB')
