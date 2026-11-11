from PIL import Image,ImageDraw,ImageFont,ImageFilter
import numpy as np, cv2, math
from scipy.ndimage import gaussian_filter
from pathlib import Path

DIR=Path('/mnt/data')
base=Image.open(DIR/'mirrored_upside_down_world_map_8K.png').convert('RGB')
W,H=base.size
w,h=1672,941
small=np.asarray(base.resize((w,h),Image.Resampling.BICUBIC),dtype=np.uint8)
# Invented plates: positions describe topology; Euler poles (lon deg, lat deg, rotation deg/Myr)
# Longitude is reflected; latitude inverted relative to a conventional north-up map.
plates=[
 ('NIVALE',      .51,.012,  25,-79, .31, (130,169,192)),
 ('KORU',        .183,.226,  90, 25, .44, (233,173,103)),
 ('LYRAN',       .026,.535, -80,-30,-.40,(73,177,168)),
 ('VEYRA',       .271,.565,  70,-39,-.55,(205,135,110)),
 ('SEREVAN',     .268,.823, -55, 62, .43,(177,150,215)),
 ('BRINEVEIL',   .448,.852, -20,  0,-.37,(83,164,209)),
 ('ANKAR',       .440,.179,  60, 12, .41,(246,188,117)),
 ('MERROW',      .523,.413, -10,-55, .49,(141,183,103)),
 ('THALASS',     .647,.726, -45,  5,-.51,(79,169,186)),
 ('CAERIN',      .676,.176, 140,-20, .38,(152,149,212)),
 ('RUSKEL',      .678,.438,  95, 40,-.43,(242,142,117)),
 ('ORREN',       .876,.221, -25,-25, .47,(199,160,116)),
 ('HALDEN',      .821,.554,  20, 49,-.58,(124,189,129)),
 ('VALLIN',      .812,.839,  75,-23,-.36,(113,173,218)),
 ('AUREOLE',     .985,.565, -110,58, .51,(171,118,206)),
 ('DRAEM',       .533,.979,  -5,-61, .34,(186,175,132)),
 ('SUNDRAL',     .267,.385, 130,  3,-.35,(112,202,184)),
]
N=len(plates)
# Periodic longitude and warped plate edges: boundaries deliberately NOT based on Earth plate data.
x=np.linspace(0,1,w,dtype=np.float32)[None,:]
y=np.linspace(0,1,h,dtype=np.float32)[:,None]
xa = (x + .019*np.sin(14*y+8*x) + .010*np.sin(26*y-17*x))%1.0
ya = y + .019*np.sin(13*x+6*y) + .013*np.sin(33*x-13*y)
dist=np.empty((N,h,w),dtype=np.float32)
for i,(_,sx,sy,*_) in enumerate(plates):
    dx=np.abs(xa-sx); dx=np.minimum(dx,1-dx)
    dist[i] = (dx*1.75)**2 + ((ya-sy)*1.02)**2
labels=dist.argmin(axis=0).astype(np.uint8)
# Polar tectonic cap one continuous fictitious plate, not Earth's Antarctic Plate.
cap = .079 + .012*np.sin(x*11.9) + .007*np.sin(x*34.4)
labels=np.where(y<cap,0,labels).astype(np.uint8)

# Local velocities from Euler rotations on a sphere; cm/year. Every plate is a rigid rotation.
lon=np.deg2rad(180.0-360.0*x)
lat=np.deg2rad(-90.0+180.0*y)
coslon=np.cos(lon); sinlon=np.sin(lon); sinlat=np.sin(lat); coslat=np.cos(lat)
vx=[];vy=[]
for _,sx,sy,p_lon,p_lat,rate,_ in plates:
    lo,la=np.deg2rad([p_lon,p_lat]); om=np.deg2rad(rate)
    wx=om*np.cos(la)*np.cos(lo); wy=om*np.cos(la)*np.sin(lo); wz=om*np.sin(la)
    v_e=637.1*(wz*coslat - sinlat*(wx*coslon+wy*sinlon))
    v_n=637.1*(wx*sinlon-wy*coslon)
    vx.append(np.broadcast_to(-v_e,(h,w)).astype('float32'))
    vy.append(np.broadcast_to(v_n,(h,w)).astype('float32'))
vx=np.stack(vx); vy=np.stack(vy)

# Classify boundaries from actual *relative* angular plate velocities.
# Continent terrain mask only helps distinguish continental collision vs oceanic convergence.
R,G,B=small[:,:,0],small[:,:,1],small[:,:,2]
land=(((G.astype(int)>B.astype(int)+15)&(G.astype(int)>R.astype(int)-30)) |
      ((R.astype(int)>125)&(B.astype(int)<185)&(G.astype(int)<210)) |
      ((R.astype(int)>205)&(G.astype(int)>205)&(B.astype(int)>205)))
land_fraction=gaussian_filter(land.astype('float32'),sigma=15)
# types: 0 convergent (subduction),1 collision,2 divergent,3 transform
masks=np.zeros((4,h,w),dtype=np.uint8)
# Relative normal speed from i (west/top) to j (east/bottom), + => expanding
for axis in (1,0):
    if axis==1:
        a=labels[:,:-1]; b=labels[:,1:]; yy,xx=np.nonzero(a!=b)
        ii=a[yy,xx]; jj=b[yy,xx]; xxr=xx+1; yyr=yy
        relative_x=vx[jj,yy,xx]-vx[ii,yy,xx]
        relative_y=vy[jj,yy,xx]-vy[ii,yy,xx]
        normal=relative_x
        lf=land_fraction[yy,xx]
    else:
        a=labels[:-1,:]; b=labels[1:,:]; yy,xx=np.nonzero(a!=b)
        ii=a[yy,xx]; jj=b[yy,xx]; xxr=xx; yyr=yy+1
        relative_x=vx[jj,yy,xx]-vx[ii,yy,xx]
        relative_y=vy[jj,yy,xx]-vy[ii,yy,xx]
        normal=relative_y
        lf=land_fraction[yy,xx]
    mag=np.hypot(relative_x,relative_y)
    transform=np.abs(normal)<np.maximum(.90,mag*.28)
    typ=np.where(transform,3,np.where(normal>0,2,np.where(lf>.72,1,0)))
    for k in range(4):
        sel=typ==k
        masks[k, yy[sel],xx[sel]]=255
        masks[k, yyr[sel],xxr[sel]]=255
# uniform stroke, filled gaps, differentiate with glows
for k in range(4):
    masks[k]=cv2.dilate(masks[k],np.ones((3,3),np.uint8))
    masks[k]=cv2.morphologyEx(masks[k],cv2.MORPH_CLOSE,np.ones((3,3),np.uint8))

# Plate shading over ORIGINAL terrain
shaded=np.asarray(Image.new('RGB',(w,h)),dtype=np.uint8).copy()
for i,(_,_,_,_,_,_,rgb) in enumerate(plates):
    shaded[labels==i]=rgb
shaded=Image.fromarray(shaded).resize((W,H),Image.Resampling.NEAREST)
base=Image.blend(base,shaded,0.095) # retain original geography
# dark glaze to make bright neon annotation legible
base=Image.blend(base,Image.new('RGB',(W,H),'#0b223b'),0.10)

color=['#fc5b62','#ffd17d','#37e4e9','#dc95ff']
for k in range(4):
    ma=Image.fromarray(masks[k]).resize((W,H),Image.Resampling.BILINEAR)
    glow=ma.filter(ImageFilter.GaussianBlur(radius=25))
    fog=Image.new('RGBA',(W,H),color[k]+ '00'); fog.putalpha(glow.point(lambda a:int(a*.48)))
    base=Image.alpha_composite(base.convert('RGBA'),fog)
    # crisp core
    core=Image.new('RGBA',(W,H),color[k]+'00'); core.putalpha(ma.point(lambda a:int(a*.94)))
    base=Image.alpha_composite(base,core)

# Draw SVG-style annotations at full resolution
im=base.convert('RGBA')
d=ImageDraw.Draw(im,'RGBA')
font='/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'
bold='/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf'
serif='/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf'
Ftitle=ImageFont.truetype(bold,86)
Fsub=ImageFont.truetype(font,41)
Fhead=ImageFont.truetype(bold,46)
Flabel=ImageFont.truetype(bold,50)
Fmeta=ImageFont.truetype(font,35)
Fleg=ImageFont.truetype(font,39)

def panel(b, radius=36):
    d.rounded_rectangle(b,radius=radius,fill=(8,22,39,211),outline=(182,228,240,96),width=3)

# top-left title and explanation
panel((85,85,2830,510),42)
d.text((155,144),'THE REIMAGINED PLATES',font=Ftitle,fill=(243,248,251,255))
d.text((160,264),'A fictional global tectonic reconstruction',font=Fsub,fill=(181,221,234,255))
d.text((160,349),'17 independent plates  •  original plate boundaries',font=Fsub,fill=(219,228,231,255))
d.text((160,405),'Upside-down + mirrored projection',font=Fsub,fill=(219,228,231,255))
# Legend in lower-left water
panel((85,3270,1890,4235),38)
d.text((154,3330),'BOUNDARY TYPES',font=Fhead,fill=(250,250,246,255))
leg=[('Convergent / subduction',color[0]),('Continental collision',color[1]),('Divergent / spreading',color[2]),('Transform / strike-slip',color[3])]
for idx,(text,c) in enumerate(leg):
    yy=3460+idx*126
    d.line((160,yy+20,380,yy+20),fill=c,width=18)
    d.text((430,yy-12),text,font=Fleg,fill=(244,246,248,255))
d.line((152,3980,1816,3980),fill=(177,214,222,112),width=2)
d.text((158,4010),'Arrows: plate movement (schematic)',font=Fmeta,fill=(215,234,241,255))
d.text((158,4075),'Colors: a calculated relative-motion snapshot',font=Fmeta,fill=(215,234,241,255))

# Label positions inside selected regions (normalized); original plate names only.
# Label may be shifted to open ocean to maintain readability.
manual={
 'NIVALE':(.50,.035), 'KORU':(.171,.246), 'LYRAN':(.040,.542),
 'VEYRA':(.295,.603),'SEREVAN':(.253,.818),'BRINEVEIL':(.449,.826),
 'ANKAR':(.434,.194),'MERROW':(.523,.407),'THALASS':(.632,.723),
 'CAERIN':(.678,.164),'RUSKEL':(.679,.435),'ORREN':(.862,.232),
 'HALDEN':(.837,.584),'VALLIN':(.814,.820),'AUREOLE':(.962,.558),
 'DRAEM':(.510,.941),'SUNDRAL':(.271,.393)
}
# centroid velocity evaluated at label site from same Euler law
for i,(name,sx,sy,p_lon,p_lat,rate,rgb) in enumerate(plates):
    lx,ly=manual[name]
    X=int(lx*W); Y=int(ly*H)
    # For polar cap, title overlaps; label toward center polar section
    if name=='NIVALE': X,Y=int(.51*W),190
    # Velocities at chosen normalized point
    lo=np.deg2rad(180-360*lx); la=np.deg2rad(-90+180*ly)
    polon,polat=np.deg2rad([p_lon,p_lat]); om=np.deg2rad(rate)
    wx=om*np.cos(polat)*np.cos(polon); wy=om*np.cos(polat)*np.sin(polon); wz=om*np.sin(polat)
    vec=np.array([-637.1*(wz*np.cos(la)-np.sin(la)*(wx*np.cos(lo)+wy*np.sin(lo))),637.1*(wx*np.sin(lo)-wy*np.cos(lo))])
    speed=float(np.linalg.norm(vec)); norm=max(speed,.1)
    # label bbox with extra dark halo and narrow composition
    if name in ('NIVALE','LYRAN','AUREOLE'):
        fs=ImageFont.truetype(bold,43)
    else: fs=Flabel
    textstr=name+' PLATE'
    bx=d.textbbox((0,0),textstr,font=fs,stroke_width=0)
    tw=bx[2]-bx[0]
    if X-tw//2<30: X=tw//2+35
    if X+tw//2>W-30: X=W-tw//2-35
    d.text((X,Y),textstr,font=fs,anchor='mm',fill=(245,249,250,255),stroke_width=6,stroke_fill=(8,22,34,240))
    if name=='NIVALE': continue
    # draw directional arrow below name
    origin=np.array([X,Y+78],float)
    direction=vec/norm
    endpoint=origin+direction*min(185,105+speed*15)
    d.line(tuple(origin)+tuple(endpoint),fill=(252,253,252,255),width=11)
    b=direction; a=np.array([-b[1],b[0]])
    pt=[tuple(endpoint),tuple(endpoint-39*b+18*a),tuple(endpoint-39*b-18*a)]
    d.polygon(pt,fill=(252,253,252,255))
    # relative velocity rate, speed at marker
    d.text((X,Y+151),f'{speed:.1f} cm/yr',font=Fmeta,anchor='mm',fill=(246,251,253,255),stroke_width=4,stroke_fill=(8,19,27,218))

# Right top inset: geological warning (rather than asserting Earth's geology can stay unchanged)
panel((5745,90,7600,400),28)
d.text((5825,138),'HYPOTHETICAL KINEMATICS',font=Fhead,fill=(250,250,250,255))
d.text((5825,220),'Not based on modern Earth plate data',font=Fmeta,fill=(198,226,236,255))
d.text((5825,278),'Existing terrain is a visual reference only',font=Fmeta,fill=(198,226,236,255))

out=DIR/'reimagined_tectonics_8k.png'
im.convert('RGB').save(out,compress_level=1)
prev=DIR/'reimagined_tectonics_preview.png'
im.convert('RGB').resize((1920,1080),Image.Resampling.LANCZOS).save(prev)
print('Created',out,'size MB',round(out.stat().st_size/1e6,1),'resolution',im.size)
