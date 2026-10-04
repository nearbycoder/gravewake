"""Original modular Gothic landmarks for Gravewake's expanded burial grounds.

Blender is used to author, bevel, and triangulate the actual shipped geometry.
Rebuild: Blender --background --python scripts/build-landmarks.py
Engine coordinates are Y-up. Each exported asset retains a ground-level origin.
"""
import bpy, math, random, struct, json
from pathlib import Path
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'assets/architecture'; OUT.mkdir(parents=True, exist_ok=True)
bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete(use_global=False)
bpy.context.preferences.filepaths.save_version = 0
SCENE = bpy.context.scene
PALETTE = {
    'limestone': ((.30, .33, .29), 1), 'cutstone': ((.41, .43, .36), 1),
    'darkstone': ((.16, .20, .18), 1), 'moss': ((.07, .105, .061), 0),
    'iron': ((.15, .13, .10), 3), 'verdigris': ((.23, .38, .29), 3),
    'bronze': ((.46, .29, .13), 3), 'shadow': ((.032, .045, .038), 0),
    'ivy': ((.095, .14, .075), 0), 'slate': ((.20, .24, .25), 1),
}
MATS = {}
for name, (rgb, engine) in PALETTE.items():
    mat = bpy.data.materials.new(name); mat.diffuse_color = (*rgb, 1)
    mat.use_nodes = True; mat['engine_material'] = engine
    shader = mat.node_tree.nodes.get('Principled BSDF')
    shader.inputs['Base Color'].default_value = (*rgb, 1)
    shader.inputs['Roughness'].default_value = .87
    if name in ('iron', 'verdigris', 'bronze'): shader.inputs['Metallic'].default_value = .48
    MATS[name] = mat
parts = []; current = None; assets = []; manifests = []

def bcoord(p): return (p[0], -p[2], p[1])
def ecoord(p): return (p[0], p[2], -p[1])

def collection(name):
    global current, parts
    current = bpy.data.collections.new(name); SCENE.collection.children.link(current); parts = []

def mesh(name, verts, faces, mat='limestone', bevel=0, smooth=False):
    me = bpy.data.meshes.new(name); me.from_pydata([bcoord(p) for p in verts], [], faces); me.update()
    obj = bpy.data.objects.new(name, me); current.objects.link(obj); parts.append(obj)
    obj.data.materials.append(MATS[mat])
    for face in me.polygons: face.use_smooth = smooth
    if bevel:
        mod = obj.modifiers.new('Worn masonry arrises', 'BEVEL'); mod.width = bevel; mod.segments = 1
    return obj

def box(name, center, size, mat='limestone', bevel=.035):
    x, y, z = center; a, b, c = [v / 2 for v in size]
    verts = [(x+i*a, y+j*b, z+k*c) for i,j,k in [(-1,-1,-1),(1,-1,-1),(1,1,-1),(-1,1,-1),(-1,-1,1),(1,-1,1),(1,1,1),(-1,1,1)]]
    return mesh(name, verts, [(0,3,2,1),(4,5,6,7),(0,1,5,4),(2,3,7,6),(1,2,6,5),(3,0,4,7)], mat, bevel)

def extrude(name, outline, depth, mat='limestone', z=0, bevel=.025):
    n = len(outline); verts = [(x,y,z+d) for d in (-depth/2, depth/2) for x,y in outline]
    faces = [tuple(reversed(range(n))), tuple(range(n,n*2))]
    faces += [(i,(i+1)%n,(i+1)%n+n,i+n) for i in range(n)]
    return mesh(name, verts, faces, mat, bevel)

def sweep(name, points, radius, mat='cutstone', sides=8):
    points = list(map(Vector, points)); verts = []
    radii = radius if isinstance(radius, list) else [radius]*len(points)
    for k, (p,r) in enumerate(zip(points,radii)):
        axis = (points[min(k+1,len(points)-1)]-points[max(0,k-1)]).normalized()
        ref = Vector((0,1,0)) if abs(axis.y)<.9 else Vector((1,0,0))
        u = axis.cross(ref).normalized(); v = axis.cross(u)
        for i in range(sides): verts.append(p+(u*math.cos(i*math.tau/sides)+v*math.sin(i*math.tau/sides))*r)
    faces = [tuple(reversed(range(sides))),tuple(range((len(points)-1)*sides,len(points)*sides))]
    for k in range(len(points)-1):
        for i in range(sides): faces.append((k*sides+i,k*sides+(i+1)%sides,(k+1)*sides+(i+1)%sides,(k+1)*sides+i))
    return mesh(name,verts,faces,mat,smooth=True)

def lathe(name, rings, mat='cutstone', sides=12, center=(0,0,0), smooth=False):
    x,y,z = center
    verts = [(x+r*math.cos(i*math.tau/sides),y+h,z+r*math.sin(i*math.tau/sides)) for h,r in rings for i in range(sides)]
    faces = []
    for k in range(len(rings)-1):
        for i in range(sides): faces.append((k*sides+i,(k+1)*sides+i,(k+1)*sides+(i+1)%sides,k*sides+(i+1)%sides))
    return mesh(name,verts,faces,mat,smooth=smooth)

def pointed_points(x, spring, width, rise, z=0, steps=12):
    w=width/2
    left=[(x+w-2*w*math.cos(i*math.pi/(3*steps)), spring+rise*math.sin(i*math.pi/(3*steps))/math.sin(math.pi/3), z) for i in range(steps+1)]
    return left+[(2*x-p[0],p[1],p[2]) for p in reversed(left[:-1])]

def arch(name, x, spring, width, rise, thickness=.32, depth=.9, z=0, steps=12, trim=True):
    pts=pointed_points(x,spring,width,rise,z,steps)
    for i,(a,b) in enumerate(zip(pts,pts[1:])):
        a=Vector(a);b=Vector(b);t=(b-a).normalized();n=Vector((-t.y,t.x,0))*thickness
        aa=a+t*.013;bb=b-t*.013
        extrude(name+' voussoir %02d'%i,[(p.x,p.y) for p in [aa,bb,bb+n,aa+n]],depth,'cutstone',z,.018)
    if trim:
        for side in [-1,1]:
            sweep(name+' continuous moulding',[(a,b,z+side*(depth/2+.045)) for a,b,_ in pts],.07,'cutstone',6)
    return pts

def column(name,x,z,height,r=.30):
    # Square plinth, polygonal torus, shaft entasis and carved capital.
    box(name+' plinth',(x,.13,z),(r*3,.26,r*3),'darkstone',.045)
    lathe(name+' base and shaft',[(.22,r*1.40),(.35,r*1.40),(.44,r*1.08),(.56,r*.9),(.66,r*.78),(height*.55,r*.73),(height-.55,r*.70),(height-.42,r*.86),(height-.32,r*1.2),(height-.16,r*1.35)],sides=10,center=(x,0,z))
    box(name+' abacus',(x,height-.08,z),(r*2.85,.16,r*2.85),'cutstone',.025)
    for s in [-1,1]:
        sweep(name+' leaf-carved capital',[(x+s*r*.5,height-.49,z-r*.7),(x+s*r*1.03,height-.30,z-r*.97),(x+s*r*.81,height-.16,z-r*.80)],.055,'cutstone',5)

def masonry(name,x0,x1,y0,y1,z,depth,seed=1,course=.60):
    rng=random.Random(seed);row=0;y=y0
    while y < y1-.02:
        h=min(course,y1-y);cuts=[x0];xx=x0+(.58 if row%2 else 1.2)
        while xx < x1-.1: cuts.append(xx);xx+=rng.uniform(1.0,1.45)
        cuts.append(x1)
        for a,b in zip(cuts,cuts[1:]):
            box(name+' course %d'%row,((a+b)/2,y+h/2,z),(b-a-.025,h-.025,depth+rng.uniform(-.025,.025)),'limestone',.032)
        y+=h;row+=1

def polygon_masonry(name, outline, depth, z=0):
    # Clip real ashlar courses to a broken/arched outline; no painted-on joints.
    def clip(poly, axis, bound, sign):
        result=[]
        for a,b in zip(poly,poly[1:]+poly[:1]):
            ain=sign*(a[axis]-bound)>=-1e-7;bin=sign*(b[axis]-bound)>=-1e-7
            if ain:result.append(a)
            if ain!=bin:
                t=(bound-a[axis])/(b[axis]-a[axis]);result.append((a[0]+t*(b[0]-a[0]),a[1]+t*(b[1]-a[1])))
        return result
    y=min(p[1] for p in outline);row=0
    while y<max(p[1] for p in outline):
        x=math.floor(min(p[0] for p in outline))-1+(row%2)*.6
        while x<max(p[0] for p in outline):
            poly=outline
            for axis,bound,sign in [(0,x,1),(0,x+1.2,-1),(1,y,1),(1,y+.56,-1)]:
                if poly:poly=clip(poly,axis,bound,sign)
            if len(poly)>2:
                cx=sum(p[0] for p in poly)/len(poly);cy=sum(p[1] for p in poly)/len(poly)
                poly=[(cx+(a-cx)*.99,cy+(b-cy)*.974) for a,b in poly]
                area=abs(sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(poly,poly[1:]+poly[:1])))/2
                if area>.008:extrude(name,poly,depth,'limestone',z,.018)
            x+=1.2
        y+=.56;row+=1

def ivy(name,x,y,z,height,seed):
    rng=random.Random(seed)
    for strand in range(3):
        pts=[]
        for i in range(8):
            p=(x+strand*.18+math.sin(i*.95+strand)*.10,y+height*i/7,z+math.sin(i*1.4)*.035);pts.append(p)
            for side in [-1,1]:
                u,v,w=p;size=rng.uniform(.10,.19)
                mesh(name+' ivy leaf',[(u,v,w-.022),(u+side*size*.55,v+size*.8,w),(u+side*size*1.15,v+size*.3,w-.016),(u+side*size*.45,v-size*.3,w)],[(0,1,2),(0,2,3)],'ivy')
        sweep(name+' ivy stem',pts,.013,'moss',5)

def export(name, collision):
    payload=bytearray();count=0;low=Vector((999,999,999));high=-low
    dg=bpy.context.evaluated_depsgraph_get()
    for obj in parts:
        evaluated=obj.evaluated_get(dg);me=evaluated.to_mesh();me.calc_loop_triangles()
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
    assert count//3 < 20000, (name,count//3)
    (OUT/(name+'.gwe')).write_bytes(b'ENV1'+struct.pack('<I',count)+payload)
    manifests.append(dict(name=name,triangles=count//3,bounds=[list(low),list(high)],collision=collision,bytes=len(payload)+8))
    assets.append((name,current,list(parts)))
    print('LANDMARK',name,count//3,'triangles',list(low),list(high),flush=True)

# The chapel portal is genuinely open: six metres at ground level.
collection('chapel-facade')
for s in [-1,1]:
    a,b=sorted((s*3.0,s*8.80));masonry('West-front ashlar',a,b,0,5.4,0,1.12,13)
    box('Tower pier footing',(s*7.75,.17,0),(2.45,.34,1.36),'darkstone',.055)
    for z in [-.44,.44]:
        column('Portal clustered shaft',s*3.38,z,4.15,.20)
        column('Outer engaged shaft',s*8.4,z,6.4,.20)
    for level in [2.5,5.35,6.35]: box('Buttress weathering',(s*7.75,level,.05),(2.3,.22,1.35),'cutstone',.045)
    extrude('Receding tower shoulder',[(s*6.75,5.3),(s*8.75,5.3),(s*8.37,7.2),(s*7.13,7.2)],1.20,'limestone',0)
    lathe('Octagonal pinnacle',[(6.3,.63),(6.55,.72),(6.70,.48),(8.2,.43),(8.35,.54),(8.5,.43),(10.25,.02)],'cutstone',8,(s*7.75,0,0))
    sweep('Pinnacle iron finial',[(s*7.75,10.0,0),(s*7.75,10.90,0)],[.038,.015],'iron',6)
    arch('Blind side niche',s*7.25,1.9,1.12,1.45,.13,.15,-.61,7)
    ivy('Portal climbing ivy',s*8.48,.5,-.68,3.4,29+(s+1))
arch('Main pointed portal',0,4.15,6.0,5.45,.65,1.10,0,18)
for off in [.18,.37]:
    sweep('Triple archivolt',[(x,y+off,-.625) for x,y,z in pointed_points(0,4.15,6.0,5.45)],.065,'cutstone',7)
extrude('Portal apex keystone',[(-.28,9.28),(.28,9.28),(.43,10.3),(-.43,10.3)],1.35,'cutstone')
for s in [-1,1]:
    # The fractured gable is real masonry behind the coping, not floating roof bars.
    gable=[(3.28,5.40),(8.30,5.40),(8.30,6.50),(7.10,7.15),(.70,12.10),(.65,11.22),(.30,10.40),(.78,10.00),(1.48,9.48),(2.24,8.65),(2.78,7.45),(3.27,5.65)]
    polygon_masonry('Fractured gable ashlar',[(s*x,y) for x,y in gable],.80,.12)
    sweep('Broken gable coping',[(s*7.1,7.15,.1),(s*4.2,9.75,.1),(s*.7,12.1,.1)],[.17,.16,.13],'cutstone',7)
    # Partial blocks leave a broken roofline with true open sky behind it.
    for i in range(4):
        x=s*(1.0+i*.61);y=11.6-i*.44
        box('Exposed gable fracture',(x,y,.18),(.55,.40,.72),'limestone',.045)
export('chapel-facade',dict(boxes=[[-9,-3,-.70,.70],[3,9,-.70,.70]],note='Six metre central doorway stays open at ground level.'))

# Roofless nave wall: continuous low wall, open lancets, cut buttress profiles.
collection('chapel-wall')
masonry('Nave sill courses',-11,11,0,1.3,0,.85,17)
box('Nave sill drip',(0,1.34,0),(22,.17,1.05),'cutstone',.025)
for i,x in enumerate([-9.5,-4.75,0,4.75,9.5]):
    masonry('Nave pier',x-.49,x+.49,1.42,6.5,0,.93,31+i)
    extrude('Splayed exterior buttress',[(x-.36,0),(x+.36,0),(x+.30,4.9),(x-.30,4.9)],1.42,'limestone',0,.035)
    for side in [-1,1]:
        column('Nave engaged shaft',x,side*.52,5.45,.17)
    box('Pier weather cap',(x,6.47,0),(1.28,.22,1.4),'cutstone',.025)
    lathe('Broken nave finial',[(6.57,.33),(6.77,.34),(7.12,.19),(7.45 if i%2 else 7.23,.025)],'cutstone',6,(x,0,0))
for i,x in enumerate([-7.125,-2.375,2.375,7.125]):
    arch('Open nave lancet',x,3.7,3.73,2.26,.29,.8,0,9)
    # Y tracery splits the opening without a heavy opaque window panel.
    sweep('Window central mullion',[(x,1.4,0),(x,3.0,0),(x,4.35,0)],.084,'cutstone',6)
    for s in [-1,1]:
        sweep('Y tracery branch',[(x,3.0,0),(x+s*.57,3.72,0),(x+s*1.07,4.63,0)],.073,'cutstone',6)
    if i in [0,3]: ivy('Nave ivy',x-1.57,.5,-.49,3.1,40+i)
export('chapel-wall',dict(boxes=[[-11,11,-.75,.75]],note='Continuous stone sill is 1.4m high; windows are not ground passages.'))

collection('cloister-bay')
for s in [-1,1]:
    masonry('Arcade pier',s*3.35-.55,s*3.35+.55,0,3.45,0,.96,33)
    column('Arcade facing column',s*3.0,-.60,3.65,.22)
    column('Arcade rear column',s*3.0,.60,3.65,.22)
    box('Arcade pier footing',(s*3.35,.12,0),(1.35,.24,1.55),'darkstone',.04)
vault=arch('Cloister pointed vault',0,3.65,5.60,3.8,.45,1.1,0,14)
# Masonry spandrels meet every voussoir rather than hovering beside the vault.
for i,(a,b) in enumerate(zip(vault,vault[1:])):
    a=Vector(a);b=Vector(b);t=(b-a).normalized();n=Vector((-t.y,t.x,0))*.43
    a+=n;b+=n;edge=-4 if (a.x+b.x)<0 else 4
    extrude('Arcade supporting spandrel',[(a.x,a.y),(b.x,b.y),(edge,b.y),(edge,a.y)],1.04,'limestone',0,.004)
box('Arcade upper infill',(0,7.74,0),(8,.17,1.04),'limestone',.015)
box('Cloister crown cornice',(0,7.93,0),(8.16,.30,1.45),'cutstone',.045)
for x in [-3.55,0,3.55]:
    box('Cloister parapet fragment',(x,8.25,0),(.65,.33,.72),'limestone',.035)
ivy('Arcade ivy',3.77,.3,-.54,4.25,38)
export('cloister-bay',dict(boxes=[[-4.05,-2.55,-.82,.82],[2.55,4.05,-.82,.82]],note='5.1m wide collision passage.'))

collection('bell-tower')
for y,size,h in [(.16,6.8,.32),(.42,6.3,.20),(.61,5.85,.18)]: box('Tower stepped foundation',(0,y,0),(size,h,size),'darkstone',.065)
# Walls have both rubble-jointed ashlar courses and narrow inset arrow slits.
for face in range(4):
    start=len(parts)
    masonry('Tower ashlar face',-2.6,2.6,.7,8.4,2.35,.52,52+face,1.1)
    for y in [3.05,6.05,8.42]: box('Tower string course',(0,y,2.39),(5.55,.22,.70),'cutstone',.03)
    for y in [3.8,6.7]:
        box('Inset arrow slit',(0,y,2.63),(.24,1.1,.018),'shadow',0)
        arch('Slit hood',0,y+.55,.38,.38,.10,.18,2.70,5)
    for obj in parts[start:]: obj.rotation_euler.z = face*math.pi/2
for x in [-2.4,2.4]:
    for z in [-2.4,2.4]:
        box('Tower corner buttress',(x,4.5,z),(.8,8,.8),'limestone',.06)
        lathe('Belfry column',[(8.5,.47),(8.75,.53),(8.95,.35),(12.3,.28),(12.55,.49),(12.8,.49)],'cutstone',8,(x,0,z))
for face in range(4):
    start=len(parts)
    arch('Open belfry',0,10.45,4.5,2.4,.35,.58,2.4,8)
    box('Belfry lintel cornice',(0,13.24,2.43),(5.7,.32,.83),'cutstone',.045)
    for obj in parts[start:]: obj.rotation_euler.z = face*math.pi/2
# The weathered cast bell hangs freely within the open belfry.
lathe('Verdigris funeral bell',[(9.17,1.10),(9.26,1.22),(9.40,1.20),(9.52,.94),(9.88,.72),(10.55,.61),(10.95,.44),(11.08,.24),(11.10,0)],'verdigris',20,smooth=True)
lathe('Bell bronze lip',[(9.17,1.10),(9.22,1.23),(9.36,1.23),(9.42,1.15)],'bronze',20,smooth=True)
sweep('Bell suspension',[(0,11,0),(0,12.58,0)],.11,'iron',8)
sweep('Bell clapper',[(0,9.95,0),(0,8.98,0)],[.09,.13],'iron',8)
box('Bell oak yoke',(0,12.15,0),(4.4,.30,.32),'darkstone',.035)
for i in range(6):
    y=13.43+i*.75;r=4.10-i*.51
    roof=lathe('Overlapping slate roof course',[(y,r),(y+.13,r+.04),(y+.90,max(.12,r-.57))],'slate',4,smooth=False)
    roof.rotation_euler.z=math.pi/4
lathe('Spire socket',[(17.95,.42),(18.18,.33),(18.50,.14)],'bronze',8)
sweep('Tower iron cross stem',[(0,18.40,0),(0,20.10,0)],[.08,.035],'iron',8)
sweep('Tower iron cross arms',[(-.46,19.55,0),(.46,19.55,0)],.055,'iron',8)
ivy('Tower creeping ivy',-1.9,.5,-2.68,5.7,61)
export('bell-tower',dict(boxes=[[-3.4,3.4,-3.4,3.4]],note='Solid tower footing; belfry is decorative above 8.5m.'))

collection('memorial-fountain')
lathe('Octagonal basin foundation',[(0,0),(0,2.7),(.16,2.8),(.31,2.68),(.37,2.35)],'darkstone',12)
lathe('Dry basin wall',[(.30,2.35),(.35,2.42),(.62,2.39),(.74,2.49),(.86,2.45),(.87,2.05),(.68,1.99),(.35,1.93),(.34,0)],'cutstone',20)
for i in range(12):
    a=i*math.tau/12;x=math.cos(a)*2.28;z=math.sin(a)*2.28
    sweep('Basin raised rib',[(x,.4,z),(x*1.02,.65,z*1.02),(x*1.02,.78,z*1.02)],.045,'cutstone',5)
lathe('Central carved fountain shaft',[(.32,.48),(.44,.62),(.60,.43),(.87,.28),(1.16,.24),(1.27,.42),(1.41,.49),(1.54,.32)],'cutstone',10)
lathe('Broken central urn',[(1.45,.24),(1.54,.44),(1.82,.52),(2.06,.36),(2.15,.37),(2.18,.25),(2.03,.26),(1.78,.37)],'verdigris',14,smooth=True)
for i in range(7):
    a=i*2.3;r=1.25+.28*math.sin(i);x=math.cos(a)*r;z=math.sin(a)*r
    box('Basin fallen stone',(x,.41,z),(.21,.12,.34),'limestone',.025)
export('memorial-fountain',dict(circle_radius=2.8,note='Dry basin is a low central cover obstacle, not a traversal floor.'))

collection('ruin-wall')
masonry('Broken wall foundation',-3,3,0,.6,0,1.16,69)
for i in range(8):
    x=-2.63+i*.75;top=[3.25,3.6,2.85,2.25,1.55,1.15,1.65,2.12][i]
    masonry('Jagged wall course',x-.355,x+.355,.61,top,0,1.00,70+i)
    box('Rubble core exposed',(x,top-.16,.06),(.61,.32,1.06),'darkstone',.055)
ivy('Broken-wall ivy',-2.82,.15,-.57,2.35,79)
export('ruin-wall',dict(boxes=[[-3.05,3.05,-.67,.67]],note='Continuous ground cover wall.'))

collection('rubble')
rng=random.Random(71)
for i in range(15):
    x=rng.uniform(-1.25,1.25);z=rng.uniform(-.70,.70);h=rng.uniform(.16,.50)
    obj=box('Tumbled dressed stone',(x,h/2,z),(rng.uniform(.30,.67),h,rng.uniform(.25,.54)),'limestone',.045)
    obj.rotation_euler.z=rng.uniform(-.8,.8)
sweep('Fractured column shaft',[(-.6,.32,-.05),(.3,.48,.3),(.84,.61,.5)],[.26,.25,.21],'cutstone',10)
export('rubble',dict(note='Scattered ankle-height visual debris. No required nav collider.'))

collection('funerary-urn')
box('Urn foundation',(0,.10,0),(1.12,.20,1.12),'darkstone',.04)
box('Urn pedestal',(0,.46,0),(.67,.55,.67),'limestone',.025)
box('Urn abacus',(0,.79,0),(.88,.13,.88),'cutstone',.025)
lathe('Fluted memorial urn',[(.84,.23),(.96,.21),(1.04,.32),(1.30,.47),(1.56,.43),(1.70,.26),(1.78,.31),(1.86,.32),(1.87,.23),(1.73,.22),(1.55,.32),(1.37,.36)],'cutstone',16,smooth=True)
for s in [-1,1]:
    sweep('Funerary scroll handle',[(s*.32,1.55,0),(s*.57,1.58,0),(s*.60,1.36,0),(s*.45,1.24,0)],.062,'bronze',8)
export('funerary-urn',dict(circle_radius=.62))

collection('cemetery-obelisk')
for y,w,h in [(.10,1.65,.20),(.27,1.36,.16),(.60,1.0,.52),(1.0,1.17,.21)]: box('Memorial stepped pedestal',(0,y,0),(w,h,w),'darkstone' if y<.3 else 'cutstone',.035)
lathe('Tapered octagonal obelisk',[(1.08,.61),(1.20,.48),(3.77,.36),(4.35,.0)],'cutstone',4)
for z in [-.40,.40]:
    box('Obelisk recessed tablet',(0,.63,z),(.51,.25,.018),'shadow',0)
    sweep('Obelisk bronze inlay',[(0,1.65,z),(0,2.71,z)],.023,'bronze',6)
    sweep('Obelisk inlay crossbar',[(-.19,2.40,z),(.19,2.40,z)],.023,'bronze',6)
export('cemetery-obelisk',dict(circle_radius=1.17))

# Solid collision-matched curtain module for district walls. Windowed nave
# modules remain a separate asset so a collision wall never covers open air.
collection('curtain-wall')
masonry('Curtain ashlar',-11,11,0,4.25,0,.96,85,.66)
for x in [-10.55,-5.28,0,5.28,10.55]:
    box('Curtain battered footing',(x,.18,0),(.92,.36,1.4),'darkstone',.04)
    extrude('Tapered curtain buttress',[(x-.35,.34),(x+.35,.34),(x+.22,3.92),(x-.22,3.92)],1.26,'limestone',0,.025)
    box('Buttress sloping cap',(x,4.02,0),(.77,.22,1.37),'cutstone',.035)
box('Continuous weathering course',(0,4.27,0),(22,.21,1.19),'cutstone',.045)
box('Crowned curtain coping',(0,4.49,0),(22,.22,1.35),'cutstone',.055)
for x in [-9.9,3.6,8.7]:ivy('Curtain climbing ivy',x,.20,-.53,2.7,89+int(x))
export('curtain-wall',dict(boxes=[[-11,11,-.7,.7]],height=4.6,note='Solid continuous masonry matched to collision rectangles. No windows or passages.'))

# Clean source gallery: named collections are individually editable and reuseable.
for i,(name,coll,objs) in enumerate(assets):
    offset=Vector(((i%3)*30,(i//3)*30,0))
    for obj in objs: obj.location += offset
SCENE.world.color=(.10,.10,.10)
SCENE['notes']='Original modular Gothic architecture. Engine Y-up; editable source Z-up. Export coordinates are local, with ground at Y=0. Geometry and all materials are original.'
SCENE['asset_manifest']='assets/architecture/manifest.json'
(OUT/'manifest.json').write_text(json.dumps(dict(format='ENV1',coordinate_system='Y-up',assets=manifests),indent=2)+'\n')
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'assets/blender/mournhollow-landmarks.blend'))
print('LANDMARKS COMPLETE',sum(a['triangles'] for a in manifests),'unique triangles',flush=True)
