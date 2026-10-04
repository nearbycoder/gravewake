"""Original Blender authoring for Gravewake's ruined burial ground.
Blender is authoring-only: ENV1 triangle streams embed in the Rust executable.
Engine coordinates are Y-up; the editable Blender scene is Z-up.
Run: Blender --background --python scripts/build-environment.py
"""
import bpy, math, random, struct, json
from pathlib import Path
from mathutils import Vector
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'assets/environment'; OUT.mkdir(parents=True,exist_ok=True)
bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete(use_global=False)
bpy.context.preferences.filepaths.save_version=0
SCENE=bpy.context.scene
current=None; parts=[]; manifests=[]; assets=[]
PALETTE={
 'stone':((.28,.32,.29),1), 'edge':((.35,.37,.32),1),
 'moss':((.07,.11,.065),0), 'iron':((.14,.115,.085),3),
 'bronze':((.46,.285,.10),3), 'bark':((.20,.13,.08),4),
 'foliage':((.085,.17,.12),5), 'shadow':((.035,.047,.035),0),
 'ember':((1.15,.095,.006),9),
}
MATS={}
for key,(rgb,engine) in PALETTE.items():
 mat=bpy.data.materials.new(key); mat.diffuse_color=(*rgb,1); mat['engine_material']=engine; mat.use_nodes=True
 shader=mat.node_tree.nodes.get('Principled BSDF'); shader.inputs['Base Color'].default_value=(*rgb,1); shader.inputs['Roughness'].default_value=.86
 if key in ['iron','bronze']: shader.inputs['Metallic'].default_value=.55
 MATS[key]=mat

def bcoord(p): return (p[0],-p[2],p[1])
def ecoord(p): return (p[0],p[2],-p[1])
def collection(name):
 global current,parts
 current=bpy.data.collections.new(name); SCENE.collection.children.link(current);parts=[]

def mesh(name,verts,faces,mat,smooth=False,bevel=0):
 me=bpy.data.meshes.new(name); me.from_pydata([bcoord(p) for p in verts],[],faces); me.update()
 obj=bpy.data.objects.new(name,me);current.objects.link(obj);obj.data.materials.append(MATS[mat]);parts.append(obj)
 for p in me.polygons:p.use_smooth=smooth
 if bevel:
  mod=obj.modifiers.new('Hand-worn edges','BEVEL');mod.width=bevel;mod.segments=1
  mod.affect='EDGES'
 return obj

def box(name,p,size,mat='stone',bevel=.035,tilt=0):
 x,y,z=p; rx,ry,rz=[v*.5 for v in size]
 v=[(x+a*rx,y+b*ry,z+c*rz) for a,b,c in [(-1,-1,-1),(1,-1,-1),(1,1,-1),(-1,1,-1),(-1,-1,1),(1,-1,1),(1,1,1),(-1,1,1)]]
 faces=[(0,3,2,1),(4,5,6,7),(0,1,5,4),(2,3,7,6),(1,2,6,5),(3,0,4,7)]
 if tilt:
  v=[(x+(a-x)*math.cos(tilt)-(b-y)*math.sin(tilt),y+(a-x)*math.sin(tilt)+(b-y)*math.cos(tilt),c) for a,b,c in v]
 return mesh(name,v,faces,mat,bevel=bevel)

def sweep(name,points,radii,mat='iron',sides=6,smooth=True):
 pp=list(map(Vector,points)); verts=[]
 for k,(p,r) in enumerate(zip(pp,radii)):
  axis=(pp[min(k+1,len(pp)-1)]-pp[max(0,k-1)]).normalized()
  ref=Vector((0,1,0)) if abs(axis.y)<.9 else Vector((1,0,0))
  u=axis.cross(ref).normalized();v=axis.cross(u)
  for i in range(sides):verts.append(p+(u*math.cos(i*math.tau/sides)+v*math.sin(i*math.tau/sides))*r)
 faces=[tuple(reversed(range(sides)))]
 for k in range(len(pp)-1):
  for i in range(sides):faces.append((k*sides+i,k*sides+(i+1)%sides,(k+1)*sides+(i+1)%sides,(k+1)*sides+i))
 faces.append(tuple(range((len(pp)-1)*sides,len(pp)*sides)))
 return mesh(name,verts,faces,mat,smooth)

def lathe(name,rings,mat,sides=12,smooth=True):
 verts=[(r*math.cos(i*math.tau/sides),y,r*math.sin(i*math.tau/sides)) for y,r in rings for i in range(sides)]
 faces=[]
 for k in range(len(rings)-1):
  for i in range(sides):faces.append((k*sides+i,(k+1)*sides+i,(k+1)*sides+(i+1)%sides,k*sides+(i+1)%sides))
 return mesh(name,verts,faces,mat,smooth)

def extrude(name,outline,depth,mat='stone',bevel=.03,z=0):
 n=len(outline);verts=[(x,y,z+d) for d in [-depth*.5,depth*.5] for x,y in outline]
 faces=[tuple(reversed(range(n))),tuple(range(n,n*2))]
 for i in range(n):faces.append((i,(i+1)%n,(i+1)%n+n,i+n))
 return mesh(name,verts,faces,mat,bevel=bevel)

def export(name):
 payload=bytearray(); count=0;low=Vector((999,999,999));high=-low
 dg=bpy.context.evaluated_depsgraph_get()
 for obj in parts:
  evaluated=obj.evaluated_get(dg); me=evaluated.to_mesh();me.calc_loop_triangles()
  # Box-projected UVs are authored with the mesh and stable under placement.
  for tri in me.loop_triangles:
   if tri.area < 1e-10: continue
   for li in tri.loops:
    loop=me.loops[li];p=Vector(ecoord(obj.matrix_world@me.vertices[loop.vertex_index].co))
    normal=me.vertices[loop.vertex_index].normal if me.polygons[tri.polygon_index].use_smooth else tri.normal
    n=Vector(ecoord(obj.matrix_world.to_3x3()@normal)).normalized()
    rgb,material=PALETTE[obj.data.materials[0].name]
    axis=max(range(3),key=lambda i:abs(n[i]));pair=[(2,1),(0,2),(0,1)][axis];uv=(p[pair[0]],p[pair[1]])
    payload.extend(struct.pack('<12f',*p,*n,*rgb,float(material),*uv));count+=1
    for j in range(3):low[j]=min(low[j],p[j]);high[j]=max(high[j],p[j])
  evaluated.to_mesh_clear()
 (OUT/(name+'.gwe')).write_bytes(b'ENV1'+struct.pack('<I',count)+payload)
 manifests.append(dict(name=name,triangles=count//3,vertices=count,bounds=[list(low),list(high)],bytes=len(payload)+8))
 assets.append((name,current,list(parts)))
 print('ENVIRONMENT',name,count//3,'triangles',flush=True)

def needle_spray(name,origin,direction,length,width,rng,dense=True):
 # A volumetric serrated spear, never a horizontal polygon disk. Its raised
 # ridge and drooping outer fingers create broken, readable spruce silhouettes.
 p=Vector(origin);axis=Vector(direction).normalized();side=axis.cross(Vector((0,1,0))).normalized()
 if side.length<.1:side=Vector((1,0,0))
 if dense:
  # The near silhouette is made of separated, slender needle bundles. These
  # are closed six-triangle spindles, leaving true holes between the twigs.
  verts=[];faces=[]
  for t in [.16,.36,.57,.78]:
   for sign in [-1,1]:
    origin=p+axis*length*(t+rng.uniform(-.025,.025))
    direction=(axis*.60+side*sign*.56+Vector((0,rng.uniform(-.30,.06),0))).normalized()
    extent=length*(.58-t*.33)*rng.uniform(.85,1.1)
    tip=origin+direction*extent
    mid=origin+direction*extent*.40
    radius=width*(.30-t*.10)
    u=direction.cross(Vector((0,1,0))).normalized()
    if u.length<.1:u=side
    v=direction.cross(u).normalized();offset=len(verts)
    verts.append(origin)
    for k in range(3):
     angle=k*math.tau/3
     verts.append(mid+(u*math.cos(angle)+v*math.sin(angle))*radius)
    verts.append(tip)
    for k in range(3):
     j=(k+1)%3
     faces.extend([(offset,offset+1+j,offset+1+k),(offset+4,offset+1+k,offset+1+j)])
  return mesh(name+' needle bundles',verts,faces,'foliage')
 verts=[p,p+axis*length*.42+Vector((0,width*1.35,0)),p+axis*length+Vector((0,-width*.35,0)),p+axis*length*.40-Vector((0,width*.70,0))]
 outline=[]
 for sign in [-1,1]:
  row=[]
  for t,w in ([(.12,.28),(.29,1),(.43,.68),(.57,.87),(.69,.43),(.84,.39)] if dense else [(.14,.36),(.35,1),(.60,.75),(.84,.35)]):
   v=p+axis*length*t+side*(sign*width*w*rng.uniform(.82,1.16))+Vector((0,-width*t*.48,0))
   row.append(len(verts));verts.append(v)
  outline.extend(row if sign<0 else reversed(row))
 half=len(outline)//2;order=[0]+outline[:half]+[2]+outline[half:]
 faces=[]
 for i in range(len(order)):
  a=order[i];b=order[(i+1)%len(order)];faces.extend([(1,b,a),(3,a,b)])
 return mesh(name,verts,faces,'foliage')

def pine(name,seed,detail):
 collection(name);rng=random.Random(seed);h=10.;lean=rng.uniform(-.23,.23)
 trunk=[(lean*(y/h)**2,y,.13*math.sin(y*.3)) for y in [0,.35,1.5,3,5,7,8.6,10]]
 sweep('Tapered furrowed trunk',trunk,[.42,.35,.28,.22,.16,.11,.07,.012],'bark',9,False)
 for i in range(6):
  a=i*math.tau/6+rng.uniform(-.15,.15);d=Vector((math.cos(a),0,math.sin(a)))
  sweep('Exposed root '+str(i),[(0,.6,0),tuple(d*.45+Vector((0,.19,0))),tuple(d*rng.uniform(.85,1.2)+Vector((0,.018,0)))],[.18,.12,.016],'bark',5,False)
 layers=10 if detail else 7
 for layer in range(layers):
  y=2.7+layer*(6.35/(layers-1));rad=(10.4-y)*.31
  count=5 if detail else 4
  for branch in range(count):
   a=branch*math.tau/count+layer*1.77+rng.uniform(-.25,.25);radial=Vector((math.cos(a),0,math.sin(a)))
   side=radial.cross(Vector((0,1,0)));L=rad*rng.uniform(.77,1.16)
   by=y+rng.uniform(-.32,.32)
   base=Vector((lean*(y/h)**2,by,.13*math.sin(y*.3)))
   start=base+radial*.12;end=base+radial*L+Vector((0,-.16-.29*L,0))
   sweep('Drooping bough %02d.%02d'%(layer,branch),[start,base+radial*L*.5+Vector((0,-.16*L,0)),end],[.058*(1-layer*.07),.032,.009],'bark',5,False)
   # Upper boughs are tighter; close trees carry extra lateral fans.
   fans=1
   for f in range(fans):
    t=.30+f*(.52/max(1,fans-1));o=base+radial*L*t+Vector((0,-.36*L*t,0))
    for s in [-1,1]:
     direction=radial*.65+side*s*.60+Vector((0,rng.uniform(-.64,-.30),0))
     needle_spray('Needle fan %02d.%02d.%02d.%d'%(layer,branch,f,s),o,direction,L*(.65-t*.3),L*(.21-t*.075),rng,detail)
   needle_spray('Trailing leader',base+radial*L*.63+Vector((0,-.25*L,0)),radial+Vector((0,-.48,0)),L*.50,L*.14,rng,detail)
 # Upright narrow leader and a few dead snapped limbs break the symmetry.
 for i in range(3):
  a=i*2.1;needle_spray('Crown leader',(.03,9.15,0),(math.cos(a)*.19,1,math.sin(a)*.19),.9,.12,rng)
 for i in range(3):
  a=rng.random()*math.tau;d=Vector((math.cos(a),0,math.sin(a)));y=1.5+i*.5
  sweep('Snapped bare branch',[(0,y,0),tuple(d*.45+Vector((0,y+.1,0))),tuple(d*.67+Vector((0,y+.22,0)))],[.075,.032,.006],'bark',5,False)
 export(name)

for seed in range(3):pine('spruce-'+str(seed),431+seed,False)
pine('spruce-near',747,True)

for variant in range(3):
 collection('grave-'+str(variant))
 box('Settled plinth',(0,.09,0),(.91,.18,.47),bevel=.055)
 box('Chamfered footing',(0,.21,0),(.76,.12,.35),'edge',.04)
 if variant==0:
  # Slender ogee crown with chipped shoulders.
  outline=[(-.32,.24),(.32,.24),(.32,.91),(.25,.99),(.22,1.14),(0,1.36),(-.22,1.14),(-.27,.98),(-.32,.94)]
 elif variant==1:
  outline=[(-.12,.25),(.12,.25),(.12,.86),(.43,.86),(.43,1.06),(.12,1.06),(.12,1.36),(-.12,1.36),(-.12,1.06),(-.42,1.06),(-.42,.86),(-.12,.86)]
 else:
  outline=[(-.36,.25),(.36,.25),(.36,.88)]+[(math.cos(a)*.36,.88+math.sin(a)*.36) for a in [i*math.pi/8 for i in range(1,9)]]
 extrude('Carved headstone',outline,.20,'stone',.024)
 if variant!=1:
  # Recessed funerary tablet and a raised cross stay legible as geometry.
  extrude('Recessed tablet',[(-.23,.38),(.23,.38),(.23,.90),(0,1.12),(-.23,.90)],.012,'shadow',.006,.107)
  box('Cross stem',(0,.80,.127),(.045,.33,.035),'edge',.006)
  box('Cross arms',(0,.86,.127),(.21,.046,.035),'edge',.006)
  for i in range(3):box('Weathered inscription slot',(0,.47+i*.045,.122),(.24-i*.035,.011,.013),'stone',.002)
 else:
  # Rose medallion at the cruciform junction.
  points=[(.082*math.cos(i*math.tau/10),.96+.082*math.sin(i*math.tau/10),.126) for i in range(11)]
  sweep('Worn cross medallion',points,[.018]*len(points),'edge',5,False)
 # Root moss remains low so silhouettes and engravings read.
 for s in [-1,1]:
  extrude('Footing moss',[(-.35,.20),(-.13,.20),(-.18,.36),(-.29,.30)],.014,'moss',0,s*.155)
 export('grave-'+str(variant))

collection('wall')
rng=random.Random(21)
for row in range(5):
 # Bonding half-blocks at alternate courses remove stacked vertical seams.
 edges=[-1,-.5,.5,1] if row%2 else [-1,0,1]
 for j in range(len(edges)-1):
  l,r=edges[j:j+2];size=r-l-.028
  box('Limestone course %d block %d'%(row,j),((l+r)*.5,.24+row*.475,0),(size,.445,.71+rng.uniform(-.04,.04)),'stone',.045,rng.uniform(-.012,.012))
box('Sloped coping lower',(0,2.44,0),(2.04,.16,.89),'stone',.06)
box('Worn coping crown',(0,2.59,0),(2.05,.19,.99),'edge',.055)
for i in range(5):
 x=-.8+i*.4
 sweep('Rusty spear rail',[(x,2.62,0),(x+.007,3.53,0)],[.028,.025],'iron',6)
 extrude('Forged lance point',[(x-.065,3.49),(x,3.76),(x+.065,3.49),(x,3.54)],.058,'iron',.004)
box('Iron cross rail',(0,2.93,0),(2,.037,.051),'iron',.006)
export('wall')

collection('buttress')
# A stepped taper and projecting base are structurally distinct from a cube.
for k,(y,sy,w,d) in enumerate([(0.16,.32,1.06,1.34),(.44,.23,.86,1.14),(1.56,2.0,.65,1.02),(2.78,.42,.73,1.11),(3.09,.19,.94,1.3),(3.29,.21,.72,1.1)]):
 box('Buttress course '+str(k),(0,y,0),(w,sy,d),'edge' if k in [0,4] else 'stone',.055)
lathe('Finial foot',[(3.39,.26),(3.55,.21),(3.64,.29),(3.82,.04),(3.89,0)],'stone',8,False)
export('buttress')

collection('gate')
# Full entrance spans the same old eight-metre opening. Pointed arch is built
# from fitted radial voussoirs, with a second recessed moulding beneath.
for side in [-1,1]:
 x=side*4.5
 for k,(y,sy,w,d) in enumerate([(.18,.36,1.78,2.0),(.49,.27,1.53,1.82),(2.34,3.48,1.25,1.57),(4.18,.19,1.55,1.81),(4.36,.19,1.72,1.98)]):
  box('Gate pier course',(x,y,0),(w,sy,d),'stone',.065)
 for row in range(7):
  box('Pier joint',(x, .8+row*.47,.800),(1.23,.025,.018),'shadow',.003)
 # Narrow attached columns introduce curved side highlights.
 for offset in [-.47,.47]:
  sweep('Fluted entrance colonette',[(x+offset,.6,.88),(x+offset,4.05,.88)],[.09,.08],'stone',8)
 # Arch curve: tangent raises from the spring, meeting a Gothic apex at y7.1.
 pts=[]
 for i in range(13):
  t=i/12;u=1-t
  x1=side*(u**3*4.5+3*u*u*t*4.5+3*u*t*t*1.1)
  y=u**3*4.35+3*u*u*t*6.25+3*u*t*t*6.95+t**3*7.5
  pts.append(Vector((x1,y,0)))
 for i in range(12):
  a,b=pts[i],pts[i+1];mid=(a+b)*.5;tangent=(b-a).normalized();normal=Vector((-tangent.y,tangent.x,0))*side
  # outward direction is left/up on the left half, right/up on the right half
  if normal.y<0:normal=-normal
  gap=.024;aa=a+tangent*gap;bb=b-tangent*gap
  vertices=[aa-normal*.10,bb-normal*.10,bb+normal*.54,aa+normal*.54]
  extrude('Fitted arch voussoir',[tuple(v[:2]) for v in vertices],1.65,'edge' if i==11 else 'stone',.025)
 for offset,rad in [(-.18,.08),(.53,.085)]:
  curve=[]
  for i,p in enumerate(pts):
   tangent=(pts[min(i+1,12)]-pts[max(i-1,0)]).normalized();normal=Vector((-tangent.y,tangent.x,0))*side
   if normal.y<0:normal=-normal
   pp=p+normal*offset;pp.z=.87;curve.append(pp)
  sweep('Arch moulded rim',curve,[rad]*len(curve),'stone',7)
 # Spear gate leaves with tiny irregularities and a double curved top rail.
 for i in range(10):
  x=side*(.20+i*.405);height=4.2+1.8*(1-abs(x)/4.5)
  sweep('Gate upright',[(x,.10,0),(x+.012*math.sin(i*3),height,0)],[.028,.025],'iron',6)
  extrude('Gate spear',[(x-.065,height-.04),(x,height+.24),(x+.065,height-.04)],.06,'bronze',.004)
 for y in [1.05,2.0]:box('Gate horizontal tie',(side*2.05,y,0),(4.08,.061,.074),'iron',.007)
 for sign in [-1,1]:
  curve=[(side*(.25+i*.35),4.85+sign*.32+math.sin(i/11*math.pi)*.43,0) for i in range(12)]
  sweep('Bowed gate tracery',curve,[.025]*len(curve),'iron',6)
 # Small scrolls nestled between rails, each bent continuously.
 for i in range(4):
  x=side*(.65+i*.87)
  curve=[(x+.26*math.cos(j*math.tau/18),1.54+.29*math.sin(j*math.tau/18),.02) for j in range(19)]
  sweep('Ironwork scroll',curve,[.021]*len(curve),'iron',5)
# Keystone is tapered, not an axis-aligned block at the arch apex.
extrude('Crowned arch keystone',[(-.28,7.13),(.28,7.13),(.38,7.95),(-.38,7.95)],1.90,'edge',.045)
export('gate')

collection('brazier')
# Rim reaches y1.4, exactly matching the inner court's fire emitters.
lathe('Carved stone pedestal',[(0,.48),(.10,.48),(.17,.40),(.24,.38),(.28,.30)],'stone',8,False)
lathe('Iron baluster',[(.26,.15),(.35,.15),(.44,.075),(.73,.085),(.91,.18),(1.02,.14)],'iron',10)
for i in range(3):
 a=i*math.tau/3;d=Vector((math.cos(a),0,math.sin(a)))
 sweep('Swept bowl support',[tuple(d*.34+Vector((0,.19,0))),tuple(d*.19+Vector((0,.55,0))),tuple(d*.17+Vector((0,.82,0))),tuple(d*.37+Vector((0,1.14,0)))],[.04,.036,.025,.025],'bronze',6)
lathe('Open hammered bowl',[(.99,.12),(1.05,.25),(1.17,.39),(1.36,.46),(1.40,.46),(1.40,.40),(1.32,.38),(1.13,.18),(1.12,0)],'iron',14)
lathe('Bronze rolled rim',[(1.36,.46),(1.38,.49),(1.42,.49),(1.45,.45),(1.42,.415)],'bronze',14)
for i in range(8):
 a=i*math.tau/8;d=Vector((math.cos(a),0,math.sin(a)))
 sweep('Cage prong',[tuple(d*.40+Vector((0,1.2,0))),tuple(d*.48+Vector((0,1.51,0))),tuple(d*.43+Vector((0,1.63,0)))],[.025,.019,.006],'iron',5)
for i in range(5):
 a=i*math.tau/5;d=Vector((math.cos(a),0,math.sin(a)))
 sweep('Charred ember', [tuple(d*.25+Vector((0,1.33,0))),tuple(-d*.17+Vector((0,1.34,0)))],[.052,.039],'shadow',5,False)
export('brazier')

# Source layout: individual asset collections remain named and editable.
for i,(name,coll,objs) in enumerate(assets):
 offset=Vector(((i%4)*12,(i//4)*12,0))
 for obj in objs:obj.location+=offset
SCENE.world.color=(.09,.09,.09)
SCENE['asset_manifest']='assets/environment/manifest.json'
SCENE['notes']='Original authored assets; engine coordinates Y-up, Blender source Z-up. Near spruce uses extra branch fans. No imported meshes.'
(OUT/'manifest.json').write_text(json.dumps(dict(format='ENV1',coordinate_system='Y-up',assets=manifests),indent=2)+'\n')
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'assets/blender/mournhollow-environment.blend'))
print('ENVIRONMENT COMPLETE',sum(a['triangles'] for a in manifests),'unique triangles',flush=True)
