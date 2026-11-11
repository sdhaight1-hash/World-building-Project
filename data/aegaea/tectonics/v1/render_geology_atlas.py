import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import pandas as pd
from pathlib import Path
from matplotlib.colors import BoundaryNorm
from matplotlib.colors import ListedColormap
from matplotlib.patches import Patch
P=Path('/mnt/data/aegaea_tectonics_v1')
d=np.load(P/'aegaea_geology_native_1672x941.npz')
plates=pd.read_csv(P/'plates.csv')
cratons=pd.read_csv(P/'cratons.csv')
fig, axs=plt.subplots(2,2,figsize=(18,11),layout='constrained',facecolor='#081d2b')
for ax in axs.ravel():
 ax.set_facecolor('#092537');ax.set_xlim(0,1672);ax.set_ylim(941,0);ax.set_xticks([]);ax.set_yticks([])
 for spine in ax.spines.values():spine.set_color('#516777')
land=d['land_mask'].astype(bool)
mask=d['craton_id']>0
plate=d['plate_id'];cmap=plt.get_cmap('tab20',17)
axs[0,0].imshow(plate,cmap=cmap,interpolation='nearest',vmin=-.5,vmax=16.5)
# highlight craton edges with muted white strokes
axs[0,0].contour(mask.astype(float),levels=[.5],colors=['#ecdfc5'],linewidths=.65)
for _,r in cratons.iterrows():
 yy,xx=np.where(d['craton_id']==int(r.craton_id))
 if len(xx):
  axs[0,0].text(np.median(xx),np.median(yy),str(r['name']),ha='center',va='center',fontsize=6,color='white',fontweight='bold',path_effects=[])
axs[0,0].set_title('17 kinematic plates · 13 ancient cores',fontsize=15,color='white')

provinces=d['province_id'];cmap2=plt.get_cmap('tab20b',11)
axs[0,1].imshow(provinces,cmap=cmap2,interpolation='nearest',vmin=-.5,vmax=10.5)
axs[0,1].contour(land.astype(float),levels=[.5],colors=['white'],linewidths=.3)
axs[0,1].set_title('Geodynamic province classification',fontsize=15,color='white')

z=d['crust_thickness_km'];im=axs[1,0].imshow(z,cmap='viridis',vmin=5,vmax=60,interpolation='nearest')
axs[1,0].set_title('Crustal thickness · Moho depth (km)',fontsize=15,color='white')
cb=fig.colorbar(im,ax=axs[1,0],shrink=.60,location='right',pad=.01);cb.ax.tick_params(colors='white');cb.outline.set_edgecolor('#506678')

z=d['lithosphere_thickness_km'];im=axs[1,1].imshow(z,cmap='magma',vmin=15,vmax=225,interpolation='nearest')
axs[1,1].set_title('Lithosphere thickness · LAB depth (km)',fontsize=15,color='white')
cb=fig.colorbar(im,ax=axs[1,1],shrink=.60,location='right',pad=.01);cb.ax.tick_params(colors='white');cb.outline.set_edgecolor('#506678')
fig.suptitle('AEGAEA  /  NATIVE-GRID TECTONIC DATA ATLAS',fontsize=23,fontweight='bold',color='white')
fig.text(.5,.004,'Schematic geology · 1672 × 941 source grid · invented plate poles, not real Earth tectonics · thermal structure assumed, not simulated',ha='center',fontsize=11,color='#d0d5db')
fig.savefig(P/'tectonic_data_atlas.png',dpi=175,facecolor=fig.get_facecolor())
plt.close(fig)

# Single detailed overview: physical base, classified boundary pixels, and craton ID outlines.
from PIL import Image
import scipy.ndimage as ndi
base=np.array(Image.open('/mnt/data/reimagined_geography_physical_preview.png').convert('RGB'))
base=np.array(Image.fromarray(base).resize((1672,941),Image.Resampling.LANCZOS))
img=(base*.78).astype('uint8')
classes=d['boundary_class']
colors=[(251,93,105),(249,202,122),(49,228,232),(222,141,253)]
# boundaries overlay with narrow dilation only on original source grid
for k,color in enumerate(colors):
 mask=(classes==k)
 mask=ndi.binary_dilation(mask,iterations=1)
 img[mask]=np.array(color,dtype=np.uint8)
import cv2
for i in range(1,14):
 cm=(d['craton_id']==i).astype('uint8')
 edge=cv2.morphologyEx(cm,cv2.MORPH_GRADIENT,np.ones((3,3),np.uint8))>0
 img[edge]=[253,246,217]
Image.fromarray(img).resize((3344,1882),Image.Resampling.LANCZOS).save(P/'tectonic_boundaries_and_cratons.png')
print('Wrote visual previews')
