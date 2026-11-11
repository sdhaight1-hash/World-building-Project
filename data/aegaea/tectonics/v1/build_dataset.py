#!/usr/bin/env python3
"""Derive a reproducible, conditional (NOT dynamically reconstructed) Aegaea geology dataset.
Uses the source masks, plate Euler poles and craton parameters from the prior map generators.
"""
import ast, csv, json, math, zipfile, hashlib
from collections import defaultdict
from datetime import date
from pathlib import Path

import cv2
import numpy as np
from PIL import Image
from scipy.ndimage import distance_transform_edt, gaussian_filter
from rasterio.features import shapes
from rasterio.transform import Affine

OUT=Path(__file__).resolve().parent
ROOT=OUT/'sources'
W,H=1672,941
RADIUS_KM=6371.0
PI=np.pi
DLON=2*PI/W
DLAT=PI/H
KM_PER_DEG=PI*RADIUS_KM/180

# Literal tables are parsed, not duplicated: this is derived from the user's actual prior files.
def extract_assignment(path, key):
    tree=ast.parse(Path(path).read_text())
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(isinstance(x,ast.Name) and x.id==key for x in node.targets):
            return ast.literal_eval(node.value)
    raise KeyError(f'Could not find {key!r} in {path}')

plates=extract_assignment(ROOT/'make_fictional_tectonics.py','plates')
cratons=extract_assignment(ROOT/'add_cratons_to_fictional_world.py','cratons')
assert len(plates)==17 and len(cratons)==13
metadata=json.loads((ROOT/'fictional_cratons_metadata.json').read_text())

source=np.array(Image.open(ROOT/'mirrored_upside_down_physical_world_map.png').convert('RGB'))
R,G,B=[source[:,:,i].astype(np.int16) for i in range(3)]
sea=((B-R>72)&(B-G>35)&(B>176)).astype(np.uint8)
sea=cv2.morphologyEx(sea,cv2.MORPH_CLOSE,np.ones((2,2),np.uint8))
land=sea==0
craton_id=np.array(Image.open(ROOT/'fictional_craton_id_mask_1672x941.png'),dtype=np.uint8)
shield=np.array(Image.open(ROOT/'fictional_shields_mask_1672x941.png'))>0
mobile=np.array(Image.open(ROOT/'fictional_paleomobile_belts_mask_1672x941.png'))>0
elevation=np.load(ROOT/'reimagined_geology_heightfield_m.npy').astype(np.float32)
assert elevation.shape==(H,W)
assert np.all(craton_id[~land]==0), 'Cratons must lie entirely on land'

# Pixel-preserving plate algorithm from make_fictional_tectonics.py.
x=np.linspace(0,1,W,dtype=np.float32)[None,:]
y=np.linspace(0,1,H,dtype=np.float32)[:,None]
xa=(x+.019*np.sin(14*y+8*x)+.010*np.sin(26*y-17*x))%1
ya=y+.019*np.sin(13*x+6*y)+.013*np.sin(33*x-13*y)
plate_distance=np.empty((len(plates),H,W),np.float32)
for i,(_,sx,sy,*_) in enumerate(plates):
    dx=np.abs(xa-sx)
    plate_distance[i]=(np.minimum(dx,1-dx)*1.75)**2+((ya-sy)*1.02)**2
plate_id=plate_distance.argmin(axis=0).astype(np.uint8)
cap=.079+.012*np.sin(x*11.9)+.007*np.sin(x*34.4)
plate_id=np.where(y<cap,0,plate_id).astype(np.uint8)
del plate_distance
assert len(np.unique(plate_id))==17
assert all(metadata['craton_cores'][i]['name']==c[0] for i,c in enumerate(cratons))
# Validate cratonic masks were generated with precisely these plate labels.
for i,c in enumerate(cratons,1):
    ys,xs=np.where(craton_id==i)
    assert len(xs)>0
    actual=np.unique(plate_id[ys,xs])
    expected=next(k for k,p in enumerate(plates) if p[0]==metadata['craton_cores'][i-1]['plate'])
    assert len(actual)==1 and actual[0]==expected,(c[0],actual,expected)

# Coordinates increase with latitude DOWN the inverted map and decrease with longitude RIGHT.
# Plate Euler poles are in this atlas geocentric frame, not in the conventional on-screen ENU frame.
latrow=np.deg2rad(-90+180*np.linspace(0,1,H,dtype=np.float64))
loncol=np.deg2rad(180-360*np.linspace(0,1,W,dtype=np.float64))
row_area=(RADIUS_KM**2)*DLON* (np.sin(-PI/2+(np.arange(H)+1)*DLAT)-np.sin(-PI/2+np.arange(H)*DLAT))
assert abs(float(np.sum(row_area)*W)-4*PI*RADIUS_KM**2)<1e-5
cell_areas=np.broadcast_to(row_area[:,None],(H,W))

# Angular velocities are given in signed degrees/Ma; conversion yields east/north components in cm/yr.
omega=np.zeros((17,3),np.float64)
for i,(_,sx,sy,plon,plat,rate,color) in enumerate(plates):
    lo,la=np.deg2rad([plon,plat])
    r=np.deg2rad(rate)
    omega[i]=r*np.array([np.cos(la)*np.cos(lo),np.cos(la)*np.sin(lo),np.sin(la)])

def velocity_screen(pid, xs, ys):
    """Screen velocity: right is positive x (WEST); down is positive y (NORTH), cm/year."""
    lo=loncol[np.asarray(xs)]
    la=latrow[np.asarray(ys)]
    om=omega[pid]
    wx,wy,wz=om[...,0],om[...,1],om[...,2]
    ve=637.1*(wz*np.cos(la)-np.sin(la)*(wx*np.cos(lo)+wy*np.sin(lo)))
    vn=637.1*(wx*np.sin(lo)-wy*np.cos(lo))
    return -ve,vn

# Class: 0 subduction (non-continental-continental convergence), 1 continental
# collision, 2 spreading/rift, 3 transform; 255 = not boundary.
lf=gaussian_filter(land.astype(np.float32),8)
boundary_class=np.full((H,W),255,np.uint8)
# Raster boundary classes are single-sided for unambiguous counting; adjacent edges supplied separately.
boundary_proximity_masks=np.zeros((4,H,W),np.uint8)
contacts=defaultdict(lambda: {'km':0.,'speed_weighted':0.,'normal_weighted':0.,'max_speed':0.,'edges':0,'land_km':0.,'ocean_km':0.,'lonlat':[]})
edges_by_group=defaultdict(list)
all_edges=[]
# Avoid fractional km rounding at per-edge level for meaningful segment lengths.
def register_edges(yy,xx,uu,vv,norm_axis):
    i=plate_id[yy,xx]; j=plate_id[vv,uu]
    keep=i!=j
    yy,xx,uu,vv,i,j=[z[keep] for z in (yy,xx,uu,vv,i,j)]
    vi=velocity_screen(i,xx,yy)
    vj=velocity_screen(j,xx,yy)
    rx=vj[0]-vi[0]; ry=vj[1]-vi[1]
    n=rx if norm_axis=='x' else ry
    speed=np.hypot(rx,ry)
    typ=np.where(np.abs(n)<np.maximum(.9,.28*speed),3,np.where(n>0,2,np.where(lf[yy,xx]>.72,1,0))).astype(np.uint8)
    # Width of each boundary grid edge in kilometres on a sphere (no projection-area fallacy).
    segment_km=np.full(len(xx),RADIUS_KM*DLAT) if norm_axis=='x' else RADIUS_KM*DLON*np.cos(latrow[yy])
    # longitude seam has both plates adjacent in the periodic source, represented at x=W.
    for p in range(len(xx)):
        a=int(i[p]);b=int(j[p]); t=int(typ[p]);yp=int(yy[p]);xp=int(xx[p]); vp=int(vv[p]);up=int(uu[p]);
        key=(min(a,b),max(a,b),t)
        if norm_axis=='x':
            boundary_x=xp+1
            pt0=(boundary_x,yp);pt1=(boundary_x,yp+1)
        else:
            pt0=(xp,yp+1);pt1=(xp+1,yp+1)
        rec=(pt0,pt1,float(segment_km[p]),float(speed[p]),float(n[p]),yp,xp)
        edges_by_group[key].append(rec)
        d=contacts[key]; dist=float(segment_km[p]);d['km']+=dist;d['speed_weighted']+=dist*float(speed[p]);d['normal_weighted']+=dist*float(n[p]);d['max_speed']=max(d['max_speed'],float(speed[p]));d['edges']+=1
        is_l=bool(land[yp,xp]);d['land_km' if is_l else 'ocean_km']+=dist
        boundary_proximity_masks[t,yp,xp]=1
        boundary_proximity_masks[t,vp,up]=1
        boundary_class[yp,xp]=t
    return len(xx)

# Segment endpoints use integer grid vertices, not pixel centers.
yh,xh=np.nonzero(plate_id[:,:-1]!=plate_id[:,1:]); nh=register_edges(yh,xh,xh+1,yh,'x')
yv,xv=np.nonzero(plate_id[:-1,:]!=plate_id[1:,:]); nv=register_edges(yv,xv,xv,yv+1,'y')
ys=np.where(plate_id[:,-1]!=plate_id[:,0])[0]; seam=register_edges(ys,np.full_like(ys,W-1),np.zeros_like(ys),ys,'x')
assert nh+nv+seam>100

# Calculate proximity and physically-motivated synthetic material fields.
sub,coll,ridge,transform=boundary_proximity_masks
proximity_px=np.stack([distance_transform_edt(m==0).astype(np.float32) for m in boundary_proximity_masks])
# World map is equirectangular; metric conversion remains approximate in east-west directions.
latfac=np.sqrt((1+np.cos(latrow)**2)/2).astype('float32')
px_km=PI*RADIUS_KM/H
proximity_km=(proximity_px*px_km*latfac[None,:,None]).astype(np.float32)
# Prior terrain generator used the pure pixel-distance proxy 0.88 Ma/px; preserve provenance.
age_proxy=np.minimum(200.,proximity_px[2]*.88).astype(np.float32)
age_proxy[land]=np.nan

# Terrain-conditioned province domain: 0 ocean basin,1 ocean ridge,2 ocean trench,
# 3 continent background,4 shield,5 platform,6 paleo-mobile belt,7 collision orogen,
# 8 volcanic arc,9 continental rift,10 transform shear zone.
province=np.where(land,3,0).astype(np.uint8)
province[~land & (proximity_px[2]<9)]=1
province[~land & (proximity_px[0]<6)]=2
province[land & mobile]=6
province[land & (craton_id>0)]=5
province[land & shield]=4
province[land & (proximity_px[1]<13)]=7
province[land & (proximity_px[0]<7)]=8
province[land & (proximity_px[2]<4)]=9
province[land & (proximity_px[3]<3)]=10
# Preserve stable cores from being declared fully reworked except near actual active interfaces.
crust=np.full((H,W),7.,np.float32)
crust[land]=35.
crust[land & (craton_id>0)]=40.
crust[province==7]=np.clip(42+7*np.exp(-proximity_px[1][province==7]/5),38,64)
crust[province==9]=24.
crust[province==8]=36.
crust[province==10]=30.
crust[province==6]=37.
crust[province==4]=41.
crust[province==5]=39.
crust=np.clip(crust,6.,67.)

# Lithospheric thickness, not to be confused with crustal thickness or topographic elevation.
# Ocean proxy from age, continental age classes stylized; old cratons retain cool mantle roots.
lab=np.where(land,105.,np.minimum(125.,12+10.0*np.sqrt(np.nan_to_num(age_proxy,nan=0)+.1))).astype(np.float32)
lab[land & (craton_id>0)]=195.
for k,c in enumerate(cratons,1):
    age_ga=float(c[6]); lab[craton_id==k]=np.clip(183+(age_ga-2.5)*33,170,242)
lab[land & mobile]=125.
lab[province==7]=118.
lab[province==8]=92.
lab[province==9]=67.
lab[province==10]=85.
lab=np.maximum(lab,crust+20)
# Assumed surface conductive heat flow: cratons cool (~38-45), spreading crust hot, rift/arc warm.
heat=np.where(land,58.,45+80*np.exp(-np.nan_to_num(age_proxy,nan=180)/14)).astype(np.float32)
heat[land & (craton_id>0)]=40.
heat[land & mobile]=53.
heat[province==7]=65.
heat[province==8]=83.
heat[province==9]=89.
heat[province==10]=68.
heat[province==1]=125.
heat=np.clip(heat,25,165)

# Stable basement age ONLY where represented in user-provided craton mask.
# 0 elsewhere means 'unspecified', NOT zero-million-year-old basement.
basement_ma=np.zeros((H,W),np.uint16)
for k,c in enumerate(cratons,1): basement_ma[craton_id==k]=round(c[6]*1000)
# Sea floor proxy is a model coefficient from existing terrain, NOT directly measured age.

# Relative tectonic deformation/hazard tendency: unitless, not annual event probability.
weights=(1.0*np.exp(-(proximity_km[0]/130)**2)+.85*np.exp(-(proximity_km[1]/170)**2)
         +.55*np.exp(-(proximity_km[2]/90)**2)+.70*np.exp(-(proximity_km[3]/105)**2))
deformation=np.clip(weights/1.35,0,1).astype(np.float32)
# Compatible vertical columns: 0 km at topography is not used; depths relative to the geoid/surface.

def column_at(xx,yy,kind):
    ci=int(craton_id[yy,xx]); p=plates[int(plate_id[yy,xx])][0]
    lith=float(lab[yy,xx]); moho=float(crust[yy,xx]);hf=float(heat[yy,xx]);
    return {
        'kind':kind,'plate':p,'craton':cratons[ci-1][0] if ci else None,
        'atlas_longitude_deg':round(180-360*(xx+.5)/W,4),
        'atlas_latitude_deg':round(-90+180*(yy+.5)/H,4),
        'elevation_m':round(float(elevation[yy,xx]),1),
        'moho_depth_km_below_surface':round(moho,1),
        'lithosphere_asthenosphere_boundary_km':round(lith,1),
        'heat_flow_mW_m2':round(hf,1),
        'thermal_profile': [
            {'depth_km':d, 'temperature_C_schematic':round(15+1285*min(d/lith,1)+max(0,d-lith)*.45,1),
             'material':('crust' if d<moho else ('lithospheric_mantle' if d<lith else 'asthenosphere'))}
            for d in (0,5,10,20,35,50,75,100,125,150,175,200,225,250,300)
        ]
    }

# CSV writers and polygon geodata.
def write_csv(name,rows,cols):
    with (OUT/name).open('w',newline='',encoding='utf-8') as f:
        dw=csv.DictWriter(f,cols,extrasaction='ignore');dw.writeheader();dw.writerows(rows)

plate_rows=[]; columns=[]
for i,(name,sx,sy,elon,elat,rate,col) in enumerate(plates):
    zone=plate_id==i
    ys,xs=np.where(zone)
    lats=latrow[ys]; ws=row_area[ys]
    vx,vy=velocity_screen(np.full(len(ys),i),xs,ys)
    sp=np.hypot(vx,vy)
    a=float(np.sum(ws));al=float(np.sum(ws*land[ys,xs])); ao=a-al
    # Spherical weighted mean of valid grid points.
    cy=int(np.median(ys));cx=int(np.median(xs))
    plate_rows.append({'plate_id':i,'plate':name,'euler_pole_lon_deg':elon,'euler_pole_lat_deg':elat,
        'signed_rate_deg_per_Ma':rate,'area_million_km2':round(a/1e6,4),
        'land_million_km2':round(al/1e6,4),'ocean_million_km2':round(ao/1e6,4),
        'land_fraction':round(al/a,4),'mean_abs_speed_cm_yr':round(float(np.average(sp,weights=ws)),3),
        'max_abs_speed_cm_yr':round(float(np.max(sp)),3),
        'craton_names':';'.join(c[0] for c in cratons if metadata['craton_cores'][cratons.index(c)]['plate']==name),
        'color_rgb':','.join(map(str,col))})
    # A median point can lie off the plate for funny shapes; choose actual closest pixel.
    sel=((xs-cx)**2+(ys-cy)**2).argmin()
    columns.append(column_at(int(xs[sel]),int(ys[sel]),'plate_reference'))
write_csv('plates.csv',plate_rows,list(plate_rows[0]))

craton_rows=[]
for i,(name,cx,cy,rx,ry,ang,age_ga,surface) in enumerate(cratons,1):
    ys,xs=np.where(craton_id==i)
    total=float(row_area[ys].sum())
    shield_km2=float(row_area[ys[shield[ys,xs]]].sum())
    xi=int(round(np.mean(xs)));yi=int(round(np.mean(ys)))
    # Exact core internal point for valid column.
    k=np.argmin((xs-xi)**2+(ys-yi)**2); xi=int(xs[k]);yi=int(ys[k])
    craton_rows.append({'craton_id':i,'name':name,'plate':plates[int(plate_id[yi,xi])][0],
        'basement_age_Ga':age_ga,'area_million_km2':round(total/1e6,4),
        'shield_area_million_km2':round(shield_km2/1e6,4),
        'platform_area_million_km2':round((total-shield_km2)/1e6,4),
        'shield_fraction':round(shield_km2/total,4),
        'source_dominant_exposure':surface,
        'reference_lon_deg':round(180-360*(xi+.5)/W,4),'reference_lat_deg':round(-90+180*(yi+.5)/H,4),
        'median_elevation_m':round(float(np.median(elevation[ys,xs])),1),
        'median_moho_km':round(float(np.median(crust[ys,xs])),1),
        'median_LAB_km':round(float(np.median(lab[ys,xs])),1),
        'median_heat_flow_mW_m2':round(float(np.median(heat[ys,xs])),1),
        'tectonic_character':'ancient stable continental interior; local active-margin overprint only if near a boundary',
        'mineral_association_hypotheses':'granitoid-greenstone; iron formations; orogenic gold; older Ni-Cu; kimberlite where deep root survives'})
    columns.append(column_at(xi,yi,'craton_core'))
write_csv('cratons.csv',craton_rows,list(craton_rows[0]))

kind_names=['subduction','continental_collision','divergent_spreading','transform']
contacts_rows=[]
for key,data in sorted(contacts.items()):
    a,b,t=key; km=data['km']
    contacts_rows.append({'plate_a':plates[a][0],'plate_b':plates[b][0],'boundary_type':kind_names[t],
        'total_contact_length_km_est':round(km,1),
        'land_interaction_km_est':round(data['land_km'],1),
        'ocean_interaction_km_est':round(data['ocean_km'],1),
        'mean_relative_speed_cm_yr':round(data['speed_weighted']/km,3),
        'max_relative_speed_cm_yr':round(data['max_speed'],3),
        'mean_normal_velocity_local_cm_yr':round(data['normal_weighted']/km,3),
        'grid_edges':data['edges'],
        'modeled_products':{
            'subduction':'trenches; forearc basins; arc volcanism; deep-focus earthquake potential',
            'continental_collision':'thickened crust; fold-thrust belts; metamorphism; foreland basins',
            'divergent_spreading':'ridge basalts or continental extensional grabens; high heat flow',
            'transform':'strike-slip faults; pull-apart basins; localized seismicity'
        }[kind_names[t]],
        'status':'instantaneous inferred contact; plate geometry and rates are hypothetical'})
write_csv('plate_contacts_by_type.csv',contacts_rows,list(contacts_rows[0]))

province_names={0:'ocean_basin',1:'ocean_spreading_axis',2:'ocean_trench',3:'noncratonic_continent',
 4:'exposed_cratonic_shield',5:'sedimentary_cratonic_platform',6:'ancient_mobile_belt',7:'collision_orogen',
 8:'volcanic_arc',9:'continental_rift',10:'transform_shear_zone'}
provinces=[]
for i,name in province_names.items():
    ys,xs=np.where(province==i)
    if not len(xs):continue
    area=float(row_area[ys].sum())
    provinces.append({'province_id':i,'province':name,'area_million_km2':round(area/1e6,4),
     'elevation_median_m':round(float(np.median(elevation[ys,xs])),1),
     'crust_thickness_median_km':round(float(np.median(crust[ys,xs])),1),
     'LAB_depth_median_km':round(float(np.median(lab[ys,xs])),1),
     'surface_heatflow_median_mW_m2':round(float(np.median(heat[ys,xs])),1),
     'relative_deformation_index_median':round(float(np.median(deformation[ys,xs])),3)})
write_csv('crustal_provinces.csv',provinces,list(provinces[0]))

# Geospatial conversion (the longitude axis is screen-mirrored, and latitude is bottom-up inverted).
# These standard lon/lat GeoJSON coordinates have custom fictional atlas orientation and MUST NOT
# be silently superimposed on modern Earth layers.
aff=Affine(-360/W,0,180,0,180/H,-90)
def export_polygons(name, mask, name_lookup, field):
    feats=[]
    for geom,v in shapes(mask.astype('int16'),mask=(mask!=0),transform=aff,connectivity=4):
        feats.append({'type':'Feature','geometry':geom,'properties':{field:int(v), 'name':name_lookup[int(v)]}})
    (OUT/name).write_text(json.dumps({'type':'FeatureCollection','features':feats},separators=(',',':')))
    return len(feats)
plate_geoms=export_polygons('plate_polygons.geojson',(plate_id+1),{i+1:p[0] for i,p in enumerate(plates)},'plate_id_plus_one')
craton_geoms=export_polygons('craton_polygons.geojson',craton_id,{i+1:c[0] for i,c in enumerate(cratons)},'craton_id')

# Trace exact classified grid edges into coherent sequences, not artistic strokes.
def chains_for(group_edges):
    adjacent=defaultdict(list)
    for ix,edge in enumerate(group_edges):
        a,b,*_=edge
        adjacent[a].append(ix);adjacent[b].append(ix)
    visited=bytearray(len(group_edges))
    # At branches terminate chains; isolated loops start at arbitrary surviving edge.
    def walk(start_edge,start_node):
        nodes=[start_node];metrics=[];curr=start_node;edge_idx=start_edge
        while edge_idx is not None and not visited[edge_idx]:
            visited[edge_idx]=1
            e=group_edges[edge_idx]; n1,n2=e[:2];nxt=n2 if curr==n1 else n1
            nodes.append(nxt);metrics.append(e[2:5]);curr=nxt
            if len(adjacent[curr])!=2:break
            choices=[z for z in adjacent[curr] if not visited[z]]
            edge_idx=choices[0] if choices else None
        return nodes,metrics
    for node,elist in adjacent.items():
        if len(elist)!=2:
            for e in elist:
                if not visited[e]: yield walk(e,node)
    for e in range(len(group_edges)):
        if not visited[e]:yield walk(e,group_edges[e][0])

boundary_features=[]
for (a,b,t),edges in sorted(edges_by_group.items()):
    for nodes,metrics in chains_for(edges):
        if len(nodes)<2:continue
        arr=np.array(metrics,float)
        length=float(arr[:,0].sum())
        # Remove nearly all vertex-level jitter without losing plate geometry above ~15 km.
        # Keep all vertices for the high-fidelity dataset; typical traced chains already compact.
        coords=[[round(180-360*xx/W,5),round(-90+180*yy/H,5)] for xx,yy in nodes]
        # Sampling an entire line of one class still gives contact km from the original edges.
        boundary_features.append({'type':'Feature','geometry':{'type':'LineString','coordinates':coords},
          'properties':{'plate_a':plates[a][0],'plate_b':plates[b][0],'type':kind_names[t],
           'length_km_est':round(length,2),'mean_relative_speed_cm_yr':round(float(np.average(arr[:,1],weights=arr[:,0])),3),
           'mean_signed_normal_cm_yr':round(float(np.average(arr[:,2],weights=arr[:,0])),3),
           'edge_count':len(metrics)}})
(OUT/'boundary_segments.geojson').write_text(json.dumps({'type':'FeatureCollection','features':boundary_features},separators=(',',':')))

# Hypothetical geological chronology: broad causal phases, explicitly NOT reverse integrated locations.
events=[
 ('3450–3000','Ma','Ancient nuclei form','Syrin, Orvel, Herin, Kavra','Felsic proto-crust formation, greenstone belts, mantle-root stabilization','hypothetical oldest assigned basement'),
 ('3000–2500','Ma','Late Archean crust growth','Maren, Kevar, Eiden, Seldar, Ilthen, Nalor, Vorun','Growth and annealing of additional cratonic blocks; stabilization predates most younger sutures','age-constrained only by assigned basement ages'),
 ('2500–1800','Ma','Proto-continental amalgamation','all basement blocks','Mobile belts accrete onto older nuclei; deformation and high-grade metamorphism','scenario, no paleopositions'),
 ('1800–1000','Ma','Cratonic recycling and platform development','all continents','Metamorphism and sedimentary platform development on stable interiors','scenario, no radiometric control'),
 ('1000–650','Ma','Long-lived ancient suturing','multiple proto-continents','Inherited sutures and reworked continental margins; mobile belts','scenario, not present plate contacts'),
 ('650–300','Ma','Basinal sedimentation and mature continental roots','continental craton platforms','Epeiric basins, passive-margin sediments and crustal shortening locally','scenario'),
 ('300–220','Ma','Pre-breakup continental assembly','proto-Aegaean continental blocks','Assembly and weakening along inherited suture systems','scenario'),
 ('220–145','Ma','Continental extension and breakup','rift-prone younger lithosphere','Rift basins, flood basalt potential, new passive margins; resistant cratonic cores remain','inherited from earlier terrain scenario'),
 ('145–80','Ma','Oceanic-basin expansion','hypothetical spreading corridors','New oceanic crust, cooling-related subsidence and marine transgression','inherited from earlier terrain scenario'),
 ('80–25','Ma','Subduction and collision onset','modern convergent neighborhoods','Orogen growth, trench advance and arc volcanism','inherited from earlier terrain scenario'),
 ('25–0','Ma','Modern 17-plate kinematic pattern','all modern plate names','Present Euler pole motion, inferred boundaries, ongoing shortening and extension','instantaneous present motion; NOT integrated through 25 Ma')]
write_csv('tectonic_history_scenario.csv',[
 {'interval':a,'units':u,'stage':title,'associated_domains':domain,'tectonic_processes':proc,'evidence_grade':evidence}
 for a,u,title,domain,proc,evidence in events],
 ['interval','units','stage','associated_domains','tectonic_processes','evidence_grade'])

# Mineral-system hypotheses: process-based and not site-specific resource predictions.
mineral_models=[
 (4,'Archean shields','greenstone-hosted gold; banded iron formations; komatiite-associated Ni-Cu; diamond potential if deep root survives','ancient volcanism, metamorphic fluids, root stabilization','low: requires ages, geochemistry, fluid paths, burial history'),
 (5,'Craton platforms','uranium in unconformity settings; sediment-hosted copper; evaporites; hydrocarbons conditional on source rock','sedimentary burial, diagenesis, unconformity fluids','low: no basin burial or maturity model'),
 (6,'Paleo-mobile belts','orogenic gold; VMS systems; metamorphic graphite and industrial minerals','deformation and metamorphism near old sutures','low: suture ages are unspecified'),
 (7,'Collision orogens','orogenic gold; tin-tungsten granites; metamorphic gemstones','crust thickening, plutonism, fluid circulation','low: no melt differentiation or metamorphic P-T path'),
 (8,'Arc volcano belts','porphyry Cu-Mo-Au; epithermal Au-Ag; VMS potential','water-flux melting, arc magmatism, hydrothermal circulation','medium as broad association, zero local evidence'),
 (9,'Continental rifts','Ni-Cu-PGE intrusions; REE carbonatites; sedimentary Cu; geothermal potential','lithospheric thinning, decompression melting, extensional basins','low: magma chemistry not modeled'),
 (10,'Transform shear','geothermal reservoirs in releasing bends; fault-hosted hydrothermal systems','fault permeability and hydrothermal circulation','low: actual permeability not modeled'),
 (1,'Spreading centers','seafloor massive sulfide Cu-Zn; hydrothermal vent systems','oceanic magmatism and hydrothermal circulation','medium for genetic setting, no deposit prediction'),
 (2,'Deep subduction trenches','trench turbidites; accretionary-prism dewatering','subduction, sediment accretion and underplating','low: sediment supply not simulated')]
write_csv('mineral_system_hypotheses.csv',[
 {'province_id':p,'province':n,'possible_systems':m,'required_processes':process,'confidence':conf}
 for p,n,m,process,conf in mineral_models],
 ['province_id','province','possible_systems','required_processes','confidence'])
(OUT/'reference_vertical_columns.json').write_text(json.dumps(columns,indent=2))

# Native resolution compressed fields: missing age proxies are NaN; zero basement means unknown.
np.savez_compressed(OUT/'aegaea_geology_native_1672x941.npz',
 plate_id=plate_id,craton_id=craton_id,land_mask=land.astype(np.uint8),
 shield_mask=shield.astype(np.uint8),paleomobile_mask=mobile.astype(np.uint8),
 boundary_class=boundary_class, province_id=province,
 elevation_m=elevation,crust_thickness_km=crust,lithosphere_thickness_km=lab,
 heat_flow_mW_m2=heat,basement_age_ma_known=basement_ma,
 ocean_age_proxy_ma=age_proxy,deformation_tendency_0to1=deformation,
 distance_subduction_km_approx=proximity_km[0],distance_collision_km_approx=proximity_km[1],
 distance_spreading_km_approx=proximity_km[2],distance_transform_km_approx=proximity_km[3])

# Lossless GeoTIFFs with named band meanings and explicit coordinate convention (fictional atlas frame).
import rasterio
for short,arr,nodata,description,dtype in [
 ('plate_id',plate_id,255,'0..16 are stable IDs for Euler plate entries','uint8'),
 ('craton_id',craton_id,0,'0 noncratonic; 1..13 as in cratons.csv','uint8'),
 ('elevation_m',elevation,None,'positive up; modeled metres','float32'),
 ('crust_km',crust,None,'modeled depth to Moho','float32'),
 ('lithosphere_km',lab,None,'modeled depth to LAB','float32'),
 ('province_id',province,255,'0..10 as in crustal_provinces.csv','uint8'),
 ('heatflow_mWm2',heat,None,'schematic surface heat flow, not measured','float32')]:
    with rasterio.open(OUT/(short+'.tif'),'w',driver='GTiff',height=H,width=W,count=1,
       dtype=dtype,crs=None,transform=aff,compress='deflate',predictor=3 if dtype=='float32' else 2,nodata=nodata) as dst:
        dst.write(arr.astype(dtype),1)
        dst.set_band_description(1,description)
        dst.update_tags(ATLAS='AEGAEA fictional lat/lon; mirror x and invert y',
             SOURCE='Aegaea 17 Euler poles / 13 craton cores; synthetic conditional reconstruction')

summary={
 'title':'Aegaea fictional tectonic and crustal dataset v1.0',
 'planetary_reference_radius_km':RADIUS_KM,'grid_width':W,'grid_height':H,
 'eight_k_display_note':'8K display files are interpolated; these arrays have native 1672x941 cells',
 'total_planet_surface_million_km2':round(4*PI*RADIUS_KM**2/1e6,4),
 'mapped_land_million_km2':round(float(cell_areas[land].sum())/1e6,4),
 'mapped_land_fraction':round(float(cell_areas[land].sum()/(4*PI*RADIUS_KM**2)),4),
 'craton_core_land_area_million_km2':round(float(cell_areas[craton_id>0].sum())/1e6,4),
 'craton_share_land_fraction':round(float(cell_areas[craton_id>0].sum()/cell_areas[land].sum()),4),
 'plate_count':len(plates),'craton_count':len(cratons),'contact_pair_class_count':len(contacts_rows),
 'total_boundary_length_km_est':round(sum(c['total_contact_length_km_est'] for c in contacts_rows),1),
 'boundary_lengths_km_by_class':{k:round(sum(c['total_contact_length_km_est'] for c in contacts_rows if c['boundary_type']==k),1) for k in kind_names},
 'plate_polygon_features':plate_geoms,'craton_polygon_features':craton_geoms,
 'boundary_linestring_features':len(boundary_features),
 'elevation_land_percentile_m':{str(p):round(float(np.percentile(elevation[land],p)),1) for p in (0,10,25,50,75,90,99,100)},
 'bathymetry_ocean_percentile_m':{str(p):round(float(np.percentile(elevation[~land],p)),1) for p in (0,10,25,50,75,90,99,100)},
 'georeference':{'convention':'fictional inverted and horizontally mirrored equirectangular atlas','screen_x_to_lon':'lon=180-360*x/1672','screen_y_to_lat':'lat=-90+180*y/941','longitude_wrap':'at left/right boundary',
     'important':'Coordinates are synthetic atlas degrees; DO NOT overlay with real-world EPSG:4326 data; GeoTIFF CRS omitted deliberately'},
 'uncertainty':{
   'direct_from_previous_assets':['17 Euler poles and speeds','present plate seed geometry','13 craton seeds and basement ages','land/coast shape','synthetic elevation grid','source shield & paleo-mobile masks'],
   'derived_calculations':['per-cell plate ID','contact class and relative motion','plate/craton area on sphere','boundary length','craton share per plate'],
   'scenario_assumptions':['crustal thickness','thermal lithosphere thickness','heat flow','province classification thresholds','relative hazard tendency','vertical temperature profiles','deep-time chronology','mineral-system associations'],
   'missing_physics':['time-integrated plate motions, subducted slabs and mantle convection','fully coupled crustal thickening and isostasy','sediment transport and erosion','paleomagnetic/stratigraphic constraints','3D thermal and stress solution','explicit conservation of area at evolving ridges/trenches']},
 'units':{'plate_motion':'cm/year','plate_rotation':'signed degrees per Ma','length':'km','area':'million km2',
     'ages':'Ma or Ga before fictional present','depth':'km below local surface','height':'metres relative to arbitrary sea-level datum'},
 'validation':{'unique_plate_ids':int(len(np.unique(plate_id))),'all_cratons_on_land':bool(np.all(craton_id[~land]==0)),
     'no_craton_straddles_current_plate':True,'area_conserves_sphere':True,
     'native_elevation_match':bool(np.array_equal(elevation,np.load(ROOT/'reimagined_geology_heightfield_m.npy')))},
 'evidence_level':'Conditional synthetic reconstruction; numerical summaries of design, not scientifically observed geology.'
}
(OUT/'summary.json').write_text(json.dumps(summary,indent=2))
(OUT/'atlas_field_dictionary.json').write_text(json.dumps({
 'data_file':'aegaea_geology_native_1672x941.npz','row_order':'top row of image is atlas south pole, bottom row is atlas north pole',
 'plate_id':'0..16 in plates.csv','craton_id':'0 none, 1..13 as cratons.csv',
 'land_mask':'1 land, 0 ocean, fixed original coastlines','shield_mask':'1 exposed ancient cratonic basement',
 'paleomobile_mask':'1 inferred ancient basement-margin mobile zone, not active plate fault',
 'boundary_class':{'0':'subduction','1':'continental_collision','2':'divergent_spreading','3':'transform','255':'none'},
 'province_id':province_names,'elevation_m':'float32 source synthetic DEM; >0 land, <0 ocean',
 'crust_thickness_km':'float32 scenario; depth to Moho',
 'lithosphere_thickness_km':'float32 scenario; approximate lithosphere-asthenosphere boundary depth',
 'heat_flow_mW_m2':'float32 scenario; schematic conductive surface flux',
 'basement_age_ma_known':'uint16 0 unspecified, >0 age for explicitly seeded craton cores',
 'ocean_age_proxy_ma':'float32 NaN on land; NOT a rigorously reconstructed spreading age',
 'deformation_tendency_0to1':'float32 relative spatial index; NOT earthquake risk or event probability',
 'distance_*_km_approx':'estimated, approximate geographic distances to synthetic plate boundary class',
 'absolute_motion_frame':'Model Euler rotation frame, no fixed mantle frame',
 'temperature_columns':'heuristic geothermal endmember, not a conductive PDE solution'
},indent=2))

# Documentation first in the archive, with a comprehensive scientific limitations note.
readme=f'''# AEGAEA — fictional tectonics dataset v1.0

## Scope

The dataset is *derived from the exact raster and source generator files* of the previously
constructed upside-down/horizontally-mirrored Aegaea world. It contains {len(plates)} instantaneous
Euler-rotation plates and {len(cratons)} separate, ancient cratonic basement cores.

**It does not claim to be the output of a dynamically valid multi-billion-year tectonic model.**
The values for thermal structure, crustal thickness, tectonic provinces, and paleogeographic
chronology are explicit scenario assumptions with usable parameter values.

## Coordinate convention

Native raster dimensions: {W} columns x {H} rows. This is the native computational grid.
The 8K rendered image has interpolated pixels, not independent geological observations.

- At screen-left, longitude is +180 degrees, decreasing to -180 on screen-right.
- At screen-top, latitude is -90 degrees; it increases to +90 at screen-bottom.
- Hence screen-right means geographic WEST, and screen-down means NORTH.
- Coordinate formulas at grid vertices: `lon = 180 - 360*x/{W}`;
  `lat = -90 + 180*y/{H}`. Pixel-center coordinates add 0.5 to x/y.
- The longitude axis is periodic. The actual coordinates deliberately DO NOT represent
  Earth locations; do not overlay with Earth EPSG:4326 geospatial products.
- The assumed planetary mean radius is {RADIUS_KM} km; this sets kinematic and area units.

## Contents

- `summary.json` — quantitative global metrics, validation results, cautions.
- `plates.csv` — exact Euler poles/rates with calculated plate areas and speeds.
- `cratons.csv` — all 13 source cores, basin/shield split, age, area and LAB estimates.
- `plate_contacts_by_type.csv` — all adjacent plate pairs per active contact class,
   lengths, normal velocities, convergence/spreading/strike slip, broad products.
- `crustal_provinces.csv` — mapped crust/thermal provinces; numeric IDs in field dictionary.
- `tectonic_history_scenario.csv` — proposed ancient-to-modern chronology, NOT a solved reconstruction.
- `mineral_system_hypotheses.csv` — possible depositional environments, NOT resource locations.
- `plate_polygons.geojson`, `craton_polygons.geojson` — polygon geometry tied to raster IDs.
- `boundary_segments.geojson` — traced local classified boundary line segments, from actual label contacts.
- `reference_vertical_columns.json` — plate/craton reference columns to 300 km depth, heuristic geotherms.
- `aegaea_geology_native_1672x941.npz` — all 18 aligned raster layers (see `atlas_field_dictionary.json`).
- `plate_id.tif`, `craton_id.tif`, `elevation_m.tif`, `crust_km.tif`,
  `lithosphere_km.tif`, `province_id.tif`, `heatflow_mWm2.tif` —
  lossless GeoTIFF data; deliberately omit Earth CRS due to mirrored fictional axis.
- `build_dataset.py` — executable source code, uses earlier source files from /mnt/data.

## Physics and methods

### Plates, motion and contacts

Each of the 17 plates is assigned an Euler pole (lon,lat) and signed angular
rotation (deg/Ma) as defined in `make_fictional_tectonics.py`. The local rigid
velocity is `v = omega x radius_vector`; radius=6371 km. Relative velocities on
neighboring raster cells determine normal motion and boundary type. Transform
class where `|normal| < max(0.9, 0.28*|relative velocity|)` cm/year; positive
normal is extension. Negative normal is convergence, further designated
collision in mostly continental local neighborhoods, otherwise subduction.
Classification is **heuristic**, especially at narrow continental margins and
where one plate is fully oceanic. No subducting-plate polarity or slab geometry
has yet been solved.

The output summaries report one global class per local cell contact, but individual
plate pairs can have different boundary classes along their length. Their total
pixel boundary length uses spherical line metrics, so contact-length measurements
are discretization-dependent estimates, not surveyed fault-length measurements.

### Cratons

Source craton IDs/ages/exposure are retained exactly from the original craton
mask and metadata. The shield mask is a visible-exposure subset of a larger
craton, not a second separate plate. The adjacent paleo-mobile mask is a
schematic possible inherited suture belt, not evidence of a precise ancient fault.
The original generator clipped cores at present plate limits, suppressing
rifted matching craton fragments. Ancient affinity between separated cores
requires dedicated future reconstruction.

### Crust and lithosphere (additional assumptions)

Ocean crust = 7 km. Noncratonic continental crust = 35 km. Stable cratonic crust
= 39–41 km. Collisional orogens may thicken locally to ~42–50 km in this version;
rifts thin to ~24 km. Strong old cratonic mantle roots = ~186–214 km, and can
locally reach about 214 km according to age. Other continent ~105 km; active
rift ~67 km; thermal ocean lithosphere grows roughly with sqrt(age proxy).
The heat-flow defaults (~40 mW/m² cratonic, 58 continental, up to ~125 ridge)
are stylized reference values, not an energy-conserving thermal calculation.
The vertical columns use a schematic 15 C at the surface grading to 1300 C at
LAB, not geothermal PDE output. Do NOT use those values as predictive stress,
melting or geochemistry without additional calibration.

### Age and bathymetry

The existing DEM elevation is reused byte-for-byte; it is not refitted to
new crust/lithosphere parameters. Ocean age is deliberately named
`ocean_age_proxy_ma`, because it comes from pixel distance to spreading ridges
and a fixed coefficient (0.88 Ma/pixel) used by the previous terrain generator.
It is NOT a kinematic, magnetostratigraphic or paleogeographic age solution.
Example real-world half-space-cooling reference: approximate depth(m) =
2500+350*sqrt(age_Ma) only over young oceanic age ranges; it is not implemented
as a hard constraint here.

## Validation and known limitations

- Tests: all 17 source plate IDs present, all 13 cratons present and on land,
  craton masks assigned to their original single plate, global area sums to
  4*pi*R^2, and DEM unchanged.
- Not validated: stress balance, force balance, trench polarity, slab dip/depth,
  dynamic topography, oceanic crust production/conservation, erosion,
  isostatic compensation, terrain coherence with modeled Moho/LAB, or
  sediment transport.
- The terrain was generated *before* cratonic thermal/mechanical properties.
  Any physically-coupled future terrain regeneration must take those roots into account.
- Stage history is an editable hypothesis; you cannot run the present Euler pole
  model backward 3.5 billion years and expect a physically meaningful craton history.
- Mineral-system associations are broad geological possibilities, never
  verified mineral deposits or probabilities.

## Quick Python use

```python
import numpy as np
z = np.load('aegaea_geology_native_1672x941.npz')
print(z.files)
print(z['plate_id'].shape, z['craton_id'].max())
# A single location: (row 500, column 600)
print(z['elevation_m'][500,600], z['lithosphere_thickness_km'][500,600])
```

## Recommended next physically consistent step

Use this present-day model as boundary/initial conditions. Create explicitly
bounded age-stage rotations and plate topology changes, pair old cratons by
paleosuture evidence, solve subduction *polarity* and dynamic uplift, then
run an erosion/sedimentation and isostatic response model. Without that,
this model is an internally reproducible procedural hypothesis, not an
empirically established geologic chronology.
'''
(OUT/'README.md').write_text(readme)

# Checksums for reproducible build and simple archive verification.
manifest=[]
for f in sorted(OUT.iterdir()):
    if not f.is_file() or f.name=='MANIFEST.sha256':continue
    manifest.append(f'{hashlib.sha256(f.read_bytes()).hexdigest()}  {f.name}')
(OUT/'MANIFEST.sha256').write_text('\n'.join(manifest)+'\n')

print('MAP_STATS',json.dumps(summary,indent=2))
print('FILES',[(p.name,round(p.stat().st_size/1e6,2)) for p in OUT.iterdir() if p.is_file()])
