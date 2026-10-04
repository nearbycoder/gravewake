"""Author Gravewake's segmented creature roster in Blender 4.5+.

Y is up, faces look along +Z. Each named mesh belongs to one rigid rig segment.
The game drives these segments with the same pose as hit tests and ragdolls.
No Blender installation or loose assets are needed by the shipped executable.
"""
import bpy, math, struct, json, random
from pathlib import Path
from mathutils import Vector, Matrix

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'assets/enemies'
OUT.mkdir(parents=True, exist_ok=True)
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
bpy.context.preferences.filepaths.save_version = 0
random.seed(47)
scene = bpy.context.scene
materials = {}
objects = []
current = None

def material(name, color, engine=2):
    m = bpy.data.materials.new(name)
    m.diffuse_color = (*color, 1)
    m.use_nodes = True
    p = m.node_tree.nodes.get('Principled BSDF')
    p.inputs['Base Color'].default_value = (*color, 1)
    p.inputs['Roughness'].default_value = .82
    if engine in (3,11,12): p.inputs['Metallic'].default_value = .7
    if engine == 9:
        p.inputs['Emission Color'].default_value = (*color, 1)
        p.inputs['Emission Strength'].default_value = 1.0
    else:
        nodes=m.node_tree.nodes; links=m.node_tree.links
        noise=nodes.new('ShaderNodeTexNoise'); noise.inputs['Scale'].default_value=95
        bump=nodes.new('ShaderNodeBump'); bump.inputs['Strength'].default_value=.12; bump.inputs['Distance'].default_value=.002
        links.new(noise.outputs['Fac'],bump.inputs['Height']);links.new(bump.outputs[0],p.inputs['Normal'])
    m['engine_material']=engine
    materials[name]=m
    return name

material('Aged ivory',(.61,.56,.42))
material('Joint shadows',(.25,.22,.16))
material('Deep cavities',(.022,.016,.012),0)
material('Old iron',(.20,.22,.21),11)
material('Verdigris bronze',(.25,.30,.22),3)
material('Worn bronze',(.36,.25,.11),12)
material('Burial linen',(.15,.115,.075),7)
material('Oxblood cloth',(.16,.026,.03),7)
material('Charred cloth',(.042,.05,.046),7)
material('Mourning violet',(.079,.052,.11),7)
material('Mouldy shroud',(.07,.11,.075),7)
material('Wing leather',(.12,.058,.055),7)
material('Gargoyle stone',(.31,.37,.36),2)
material('Cinder bone',(.34,.24,.17))
material('Plague skin',(.24,.28,.13))
material('Soul embers',(2.5,.30,.045),9)
material('Soul verdigris',(.08,1.8,.63),9)
material('Soul violet',(.7,.16,1.8),9)
material('Sickly light',(.4,1.15,.08),9)
material('Rotten wood',(.14,.10,.065),13)

def finish(o,name,mat,group,smooth=True):
    o.name=name; o['rig_group']=group
    o.data.materials.append(materials[mat])
    for p in o.data.polygons: p.use_smooth=smooth
    for c in list(o.users_collection): c.objects.unlink(o)
    current.objects.link(o)
    objects.append(o)
    return o

def mesh(name,verts,faces,mat,group,smooth=True):
    me=bpy.data.meshes.new(name); me.from_pydata(verts,[],faces);me.update()
    o=bpy.data.objects.new(name,me);scene.collection.objects.link(o)
    return finish(o,name,mat,group,smooth)

def ellipsoid(name,p,r,mat,group,segments=16,rings=10):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=segments,ring_count=rings,location=p)
    o=bpy.context.object; o.scale=r
    bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
    return finish(o,name,mat,group)

def cut(o,cutter):
    bpy.context.view_layer.objects.active=o
    mod=o.modifiers.new('Carved opening','BOOLEAN');mod.operation='DIFFERENCE';mod.object=cutter;mod.solver='EXACT'
    bpy.ops.object.modifier_apply(modifier=mod.name)
    objects.remove(cutter);bpy.data.objects.remove(cutter,do_unlink=True)

def bevel(o,width=.005):
    bpy.context.view_layer.objects.active=o
    mod=o.modifiers.new('Soft worn edges','BEVEL');mod.width=width;mod.segments=2
    bpy.ops.object.modifier_apply(modifier=mod.name)
    return o

def tube(name,pts,radii,mat,group,sides=8):
    pts=[Vector(p) for p in pts];verts=[]; faces=[]
    if isinstance(radii,(int,float)):radii=[radii]*len(pts)
    for k,p in enumerate(pts):
        axis=(pts[min(k+1,len(pts)-1)]-pts[max(0,k-1)]).normalized()
        u=axis.cross(Vector((0,0,1)) if abs(axis.z)<.9 else Vector((1,0,0))).normalized();v=axis.cross(u)
        for i in range(sides):
            a=i*math.tau/sides
            verts.append(p+(u*math.cos(a)+v*math.sin(a))*radii[k])
    for k in range(len(pts)-1):
        for i in range(sides): faces.append((k*sides+i,k*sides+(i+1)%sides,(k+1)*sides+(i+1)%sides,(k+1)*sides+i))
    faces.extend([tuple(reversed(range(sides))),tuple(range((len(pts)-1)*sides,len(pts)*sides))])
    return mesh(name,verts,faces,mat,group)

def profile(name,rings,mat,group,sides=12):
    # Horizontal cross sections, front and rear may have different depth.
    verts=[];faces=[]
    for y,x,z,rx,rz in rings:
        for i in range(sides):
            a=i*math.tau/sides
            verts.append((x+rx*math.cos(a),y,z+rz*math.sin(a)))
    for k in range(len(rings)-1):
        for i in range(sides): faces.append((k*sides+i,(k+1)*sides+i,(k+1)*sides+(i+1)%sides,k*sides+(i+1)%sides))
    faces.extend([tuple(range(sides)),tuple(reversed(range((len(rings)-1)*sides,len(rings)*sides)))])
    return mesh(name,verts,faces,mat,group)

def skull(kind):
    bone='Cinder bone' if kind==2 else 'Gargoyle stone' if kind==9 else 'Aged ivory'
    cranium=ellipsoid('Vault of skull',(0,.065,-.022),(.165,.179,.146),bone,1,24,16)
    # Real negative volume: light can enter the orbital cavities from the front.
    for s in [-1,1]:
        cavity=ellipsoid('Orbital cutter',(s*.070,.018,.120),(.062,.054,.081),'Deep cavities',1,20,12)
        cut(cranium,cavity)
        ellipsoid('Recessed orbital darkness',(s*.070,.018,.067),(.046,.041,.023),'Deep cavities',1,12,8)
        tube('Supraorbital ridge',[(s*.019,.065,.106),(s*.053,.083,.128),(s*.100,.073,.12),(s*.136,.036,.074)],[.017,.021,.018,.010],bone,1)
        tube('Zygomatic arch',[(s*.143,.007,.04),(s*.134,-.040,.11),(s*.096,-.07,.127)],[.018,.025,.016],bone,1)
        tube('Mandibular ramus',[(s*.125,-.05,.025),(s*.116,-.132,.034),(s*.087,-.182,.119),(0,-.191,.149)],[.018,.023,.026,.020],bone,1)
        glow='Soul embers' if kind in [0,1,2,4,6] else 'Soul verdigris' if kind in [3,10] else 'Sickly light' if kind in [5,7] else 'Soul violet'
        ellipsoid('Ember deep inside orbit',(s*.070,.017,.089),(.009,.010,.010),glow,1,10,6)
    # Nasal aperture is a hole through the maxilla, with separate bridge and teeth.
    maxilla=ellipsoid('Maxilla',(0,-.078,.097),(.091,.046,.052),bone,1,16,10)
    nasal=ellipsoid('Nasal cutter',(0,-.051,.132),(.023,.034,.048),'Deep cavities',1,12,8)
    cut(maxilla,nasal)
    tube('Nasal bridge',[(0,.060,.105),(0,.014,.136),(0,-.017,.134)],[.014,.012,.008],bone,1)
    ellipsoid('Nasal shadow',(0,-.049,.091),(.019,.026,.010),'Deep cavities',1,10,6)
    for row in [0,1]:
        for i in range(10):
            a=(i-4.5)*.21
            x=math.sin(a)*.087;z=.097+math.cos(a)*.060
            top=-.105 if row==0 else -.178
            bottom=top-(.032 if row==0 else -.027)
            # Individual squared incisors and smaller side teeth, no continuous grin.
            profile('Upper tooth' if row==0 else 'Lower tooth',[(top,x,z,.010,.013),(bottom,x,z+.002,.008,.010)],bone,1,6)
    if kind in [6,9]:
        # Open-faced segmented sallet; the face and shot region remain exposed.
        for s in [-1,1]:
            tube('Helmet cheek guard',[(s*.135,.12,-.035),(s*.171,.075,.0),(s*.162,-.07,.040)],[.044,.040,.026],'Old iron',1)
        profile('Helmet crest',[(.10,0,-.035,.17,.135),(.215,0,-.039,.105,.075),(.265,0,-.04,.015,.02)],'Old iron',1)
    if kind==3:
        for s in [-1,1]:
            pts=[(s*.13,.18,-.055),(s*.23,.28,-.095),(s*.30,.42,-.10),(s*.40,.55,-.12),(s*.48,.70,-.10)]
            tube('Antler main beam',pts,[.039,.033,.026,.016,.002],bone,1,9)
            for j in range(1,4):
                p=pts[j];tube('Antler tine',[p,(p[0]-s*.055,p[1]+.075,p[2]+.018),(p[0]-s*.075,p[1]+.19,p[2]+.03)],[.022,.012,.001],bone,1,7)
        # Thorned circlet sits around the brow instead of a floating crown.
        pts=[(.164*math.cos(a*math.tau/24),.119,-.022+.137*math.sin(a*math.tau/24)) for a in range(25)]
        tube('Burial crown',pts,.012,'Worn bronze',1)
    if kind==2:
        for s in [-1,1]:
            tube('Charred horn',[(s*.13,.155,-.035),(s*.21,.25,-.065),(s*.23,.38,-.12)],[.037,.023,.002],bone,1)
        # Small cracked ember seams sit on the head; flame remains a runtime effect.
        for s in [-1,1]:tube('Cinder seam',[(s*.07,.20,.042),(s*.095,.17,.09),(s*.113,.14,.097)],[.003,.004,.002],'Soul embers',1,5)
    if kind in [8,10,11]: hood(kind)

def hood(kind):
    mat={8:'Oxblood cloth',10:'Mouldy shroud',11:'Mourning violet'}[kind]
    # Open hood: a shaped shell around an exposed face with a folded rim.
    verts=[];faces=[];n=16
    for k,(y,rx,rz,cz) in enumerate([(-.20,.19,.16,-.04),(-.04,.215,.18,-.02),(.14,.205,.19,-.025),(.29,.135,.13,-.04),(.36,.015,.022,-.09)]):
        for i in range(n+1):
            a=.19*math.pi+i/n*1.62*math.pi
            verts.append((rx*math.sin(a),y,cz+rz*math.cos(a)))
    for k in range(4):
        for i in range(n):faces.append((k*(n+1)+i,k*(n+1)+i+1,(k+1)*(n+1)+i+1,(k+1)*(n+1)+i))
    o=mesh('Folded open cowl',verts,faces,mat,1)
    bpy.context.view_layer.objects.active=o
    mod=o.modifiers.new('Cloth thickness','SOLIDIFY');mod.thickness=.008;bpy.ops.object.modifier_apply(modifier=mod.name)
    for index in [0,n]: tube('Hood rolled seam',[verts[k*(n+1)+index] for k in range(5)],[.012,.018,.014,.010,.006],mat,1)

def ribcage(kind):
    bone='Gargoyle stone' if kind==9 else 'Aged ivory'
    # Sixteen separate curved ribs, back to sternum, with a natural taper.
    for s in [-1,1]:
        for i in range(7):
            level=1.42-i*.047
            w=[.15,.196,.220,.226,.213,.189,.151][i]
            pts=[]
            for j in range(10):
                a=j/9*math.pi
                pts.append((s*w*math.sin(a),level-.031*math.sin(a)-.045*a/math.pi,-.082+.226*(1-math.cos(a))/2))
            tube('Curved rib %s %s'%(s,i),pts,[.012+.004*math.sin(j/9*math.pi) for j in range(10)],bone,0,7)
        # Scapula is a real thin blade at the back, not another sphere.
        bevel(mesh('Scapula',[(s*.055,1.40,-.115),(s*.255,1.41,-.072),(s*.105,1.23,-.115),(s*.054,1.39,-.133),(s*.253,1.40,-.088),(s*.103,1.23,-.13)],[(0,1,2),(5,4,3),(0,3,4,1),(1,4,5,2),(2,5,3,0)],bone,0),.008)
        tube('Clavicle',[(0,1.445,.035),(s*.10,1.465,.026),(s*.23,1.447,.013),(s*.33,1.40,0)],[.020,.021,.021,.022],bone,0)
    tube('Sternum',[(0,1.43,.143),(0,1.30,.153),(0,1.18,.149),(0,1.08,.104)],[.025,.020,.018,.005],bone,0)
    for i in range(10):
        y=.91+i*.055
        ellipsoid('Vertebra',(0,y,-.07),(.039,.024,.039),bone,0,10,6)
        tube('Spinous process',[(0,y,-.082),(0,y-.014,-.129)],[.012,.003],bone,0,5)
    for s in [-1,1]:
        p=ellipsoid('Ilium',(s*.111,.879,-.012),(.104,.13,.068),bone,0,16,10)
        opening=ellipsoid('Obturator foramen',(s*.096,.822,.013),(.041,.045,.11),'Deep cavities',0,12,8)
        cut(p,opening)
        tube('Pubic arch',[(s*.16,.84,.015),(s*.13,.775,.046),(s*.035,.777,.057),(0,.803,.038)],[.020,.022,.022,.017],bone,0)

def bone_segment(name,group,radius,paired=False):
    # Local Y goes from proximal to distal joint; the engine stretches only this axis.
    rings=[]
    for y,f in [(0,.86),(.045,1.15),(.12,.80),(.35,.61),(.68,.65),(.90,.89),(.97,1.15),(1,.91)]:
        rings.append((y,.006*math.sin(y*math.pi),.003*math.sin(y*math.tau),radius*f,radius*f*.86))
    profile(name,rings,'Aged ivory',group,9)
    if paired:
        profile(name+' paired shaft',[(0,-radius*.78,0,radius*.34,radius*.35),(.18,-radius*.96,.004,radius*.36,radius*.32),(.78,-radius*.93,0,radius*.32,radius*.29),(1,-radius*.72,0,radius*.44,radius*.42)],'Aged ivory',group,7)

def limbs(kind):
    for side,groups in [(-1,(2,3,4,8,9,10)),(1,(5,6,7,11,12,13))]:
        ua,la,hand,thigh,shin,foot=groups
        bone_segment('Humerus',ua,.039)
        bone_segment('Radius and ulna',la,.027,True)
        bone_segment('Femur',thigh,.048)
        bone_segment('Tibia and fibula',shin,.034,True)
        # Hands keep an actual palm, staggered fingers, and an opposing thumb.
        ellipsoid('Carpal bones',(0,.021,0),(.044,.030,.023),'Aged ivory',hand,12,8)
        for f in range(4):
            x=(f-1.5)*.023;length=[.080,.103,.096,.070][f]
            tube('Metacarpal',[(x*.65,.03,0),(x,.074,.003)],[.008,.010],'Aged ivory',hand,7)
            pts=[(x,.075,.003),(x,.075+length*.43,.007),(x,.075+length*.78,.021),(x,.075+length,.037)]
            tube('Articulated finger',pts,[.008,.007,.006,.004],'Aged ivory',hand,7)
        tube('Opposing thumb',[(side*.035,.026,0),(side*.061,.053,.014),(side*.056,.090,.039)],[.012,.009,.006],'Aged ivory',hand,8)
        ellipsoid('Calcaneus',(0,-.018,.016),(.044,.042,.065),'Aged ivory',foot,12,8)
        for toe in range(5):
            x=(toe-2)*.019;end=.20-toe*.015
            tube('Metatarsal',[(x*.45,-.01,.029),(x,-.027,.125),(x,-.035,end)],[.014,.011,.007],'Aged ivory',foot,7)
        if kind in [1,3,6,9]:
            profile('Vambrace',[(.09,0,0,.058,.051),(.25,0,0,.054,.048),(.72,0,0,.041,.037)],'Old iron' if kind!=3 else 'Verdigris bronze',la)
            profile('Greave',[(.11,0,0,.062,.058),(.27,0,.005,.065,.061),(.83,0,0,.045,.043)],'Old iron',shin)

def cloth(kind):
    mat={0:'Burial linen',1:'Charred cloth',3:'Mouldy shroud',6:'Charred cloth',7:'Burial linen',8:'Oxblood cloth',9:'Charred cloth',10:'Mouldy shroud',11:'Mourning violet'}.get(kind,'Burial linen')
    robed=kind in [8,10,11]
    n=40; levels=[(1.46,.35,.24),(1.26,.265,.205),(.98,.255,.20),(.65,.35,.41),(.21,.44,.49)] if robed else [(.96,.235,.143),(.85,.249,.16),(.58,.28,.34)]
    verts=[];faces=[]
    for k,(y,rx,rz) in enumerate(levels):
        for i in range(n):
            a=i*math.tau/n
            fold=1.+.075*math.cos(a*10)+.025*math.sin(a*17)
            yy=y if k<len(levels)-1 else y+.065*math.sin(i*2.47)+.026*math.sin(i*5.3)
            verts.append((math.cos(a)*rx*fold,yy,math.sin(a)*rz*fold-.035))
    for k in range(len(levels)-1):
        for i in range(n):
            # Slit opening at front bottom breaks the bucket silhouette.
            if k==len(levels)-2 and i in ([9] if robed else [8,9,10,11,12]):continue
            faces.append((k*n+i,k*n+(i+1)%n,(k+1)*n+(i+1)%n,(k+1)*n+i))
    o=mesh('Folded burial garment',verts,faces,mat,0)
    bpy.context.view_layer.objects.active=o
    mod=o.modifiers.new('Woven thickness','SOLIDIFY');mod.thickness=.006;bpy.ops.object.modifier_apply(modifier=mod.name)
    # Narrow belt, overlapping folds, and a hanging clasp establish scale.
    ring=[(.242*math.cos(i*math.tau/32),.941,-.035+.15*math.sin(i*math.tau/32)) for i in range(33)]
    tube('Rope cincture',ring,.014,'Burial linen',0)
    tube('Hanging rope',[(.07,.94,.13),(.04,.79,.164),(.08,.65,.177)],[.012,.011,.008],'Burial linen',0)

def armor(kind):
    mat='Verdigris bronze' if kind==3 else 'Old iron'
    profile('Forged breastplate',[(1.07,0,.015,.19,.14),(1.19,0,.02,.259,.18),(1.37,0,.005,.257,.15),(1.47,0,-.015,.16,.09)],mat,0,20)
    for s in [-1,1]:
        for i in range(3):
            p=(s*(.29+i*.043),1.41-i*.037,-.016)
            ellipsoid('Overlapping shoulder lame',p,(.114,.042,.118),mat,0,16,8)
        for i in range(5):
            ellipsoid('Peened armor rivet',(s*(.16+i*.012),1.13+i*.054,.143),(.009,.009,.006),'Worn bronze',0,8,6)
    if kind==3:
        # A restrained soul reliquary, recessed in a bronze cage, not a giant green disk.
        ellipsoid('Reliquary recess',(0,1.29,.187),(.065,.097,.025),'Deep cavities',0)
        ellipsoid('Bound soul',(0,1.29,.207),(.033,.062,.017),'Soul verdigris',0)
        for s in [-1,1]:tube('Reliquary cage',[(s*.02,1.385,.19),(s*.07,1.32,.22),(s*.067,1.24,.22),(s*.018,1.195,.19)],.009,'Worn bronze',0)
    if kind==6:
        # Curved kite shield attaches to left wrist, and separates with the arm.
        verts=[(-.22,-.24,.06),(.22,-.24,.06),(.23,.15,.08),(0,.57,.12),(-.23,.15,.08),(0,.05,.16)]
        faces=[(i,(i+1)%5,5) for i in range(5)]
        o=mesh('Convex penitence shield',verts,faces,'Old iron',4)
        bpy.context.view_layer.objects.active=o
        mod=o.modifiers.new('Shield thickness','SOLIDIFY');mod.thickness=.025;bpy.ops.object.modifier_apply(modifier=mod.name)
        tube('Shield rolled rim',verts[:5]+[verts[0]],.015,'Worn bronze',4)
        tube('Shield raised spine',[(0,-.24,.11),(0,.06,.18),(0,.55,.13)],.014,'Worn bronze',4)

def weapon_prop(kind):
    if kind==1:
        # Tang and diamond section blade, with a real hilt.
        tube('Sword grip',[(0,0,0),(0,.15,0)],.025,'Rotten wood',7,10)
        tube('Sword quillons',[(-.13,.16,0),(0,.17,0),(.13,.16,0)],.017,'Old iron',7)
        mesh('Forged diamond sword',[(0,.18,.025),(-.047,.18,0),(0,.18,-.025),(.047,.18,0),(0,.70,.018),(-.025,.70,0),(0,.70,-.018),(.025,.70,0),(0,.87,0)],[(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7),(4,5,8),(5,6,8),(6,7,8),(7,4,8)],'Old iron',7,False)
    if kind in [8,10,11]:
        tube('Gnarled staff',[(0,.61,0),(.025,.24,.01),(0,-.24,0),(-.018,-.78,.02),(0,-1.13,.018)],[.025,.029,.027,.023,.018],'Rotten wood',7,9)
        if kind==11:
            # Curved thick sickle with a tapered cutting edge.
            verts=[]
            for i in range(12):
                t=i/11; x=-.02-.71*t; y=-1.10+.22*t+.38*t*t; w=.115*(1-t)+.005
                verts.extend([(x,y-w,.008),(x,y+w,-.014),(x,y-w,-.009),(x,y+w,.014)])
            faces=[]
            for i in range(11):
                a=i*4;b=(i+1)*4
                faces.extend([(a,b,b+1,a+1),(a+2,a+3,b+3,b+2),(a,a+2,b+2,b),(a+1,b+1,b+3,a+3)])
            mesh('Curved reaping blade',verts,faces,'Old iron',7,False)
        else:
            glow='Soul verdigris' if kind==10 else 'Soul embers'
            ellipsoid('Captive lantern soul',(0,-1.06,.019),(.062,.095,.062),glow,7)
            for i in range(6):
                a=i*math.tau/6
                tube('Lantern cage',[(0,-.91,.02),(.095*math.cos(a),-1.04,.02+.095*math.sin(a)),(0,-1.20,.02)],.009,'Worn bronze',7,6)

def wing(kind,side,group):
    mat='Wing leather' if kind==4 else 'Gargoyle stone'
    # Local origin is the shoulder; sweeps and finger ribs cup the membrane.
    root=Vector((0,0,0));knuckle=Vector((side*.49,.31,-.025));tip=Vector((side*1.18,.54,-.11))
    ends=[Vector((side*.90,-.19,-.20)),Vector((side*.58,-.38,-.22)),Vector((side*.23,-.38,-.16)),Vector((side*.02,-.16,-.08))]
    tube('Wing humerus',[root,knuckle,tip],[.030,.027,.003],'Aged ivory' if kind==4 else mat,group)
    edge=[tip]+ends
    for j in range(4):
        a=edge[j];b=edge[j+1];verts=[];faces=[];steps=7
        for row in range(steps+1):
            t=row/steps
            for col in range(steps+1):
                u=col/steps
                outer=a.lerp(b,u);outer.y+=.09*math.sin(u*math.pi)
                p=knuckle.lerp(outer,t);p.z-=math.sin(t*math.pi)*math.sin(u*math.pi)*.10
                verts.append(p)
        for row in range(steps):
            for col in range(steps):
                k=row*(steps+1)+col;faces.append((k,k+1,k+steps+2,k+steps+1))
        o=mesh('Cupped scalloped wing membrane',verts,faces,mat,group)
        bpy.context.view_layer.objects.active=o
        mod=o.modifiers.new('Membrane thickness','SOLIDIFY');mod.thickness=.005;bpy.ops.object.modifier_apply(modifier=mod.name)
        tube('Wing finger rib',[knuckle,knuckle.lerp(b,.52)+Vector((0,0,-.035)),b],[.018,.011,.002],'Aged ivory' if kind==4 else mat,group,7)

def crawler():
    profile('Segmented carrion abdomen',[(.57,0,-.20,.16,.24),(.68,0,-.25,.27,.39),(.79,0,-.27,.29,.38),(.89,0,-.27,.22,.30),(.95,0,-.27,.08,.17)],'Plague skin',0,20)
    for i in range(5):
        z=-.53+i*.125
        tube('Abdominal chitin arch',[(-.24,.79,z),(-.17,.90,z),(0,.925,z),(.17,.90,z),(.24,.79,z)],[.016,.022,.025,.022,.016],'Joint shadows',0)
    for side in [-1,1]:
        for i in range(3):
            z=-.38+i*.23;g=16+(0 if side<0 else 3)+i
            tube('Hooked crawler limb',[(side*.18,.75,z),(side*.55,.78,z-.12),(side*.84,.13,z+.11),(side*.89,.045,z+.17)],[.044,.033,.014,.002],'Aged ivory',g)
        tube('Mandible',[(side*.095,.73,.12),(side*.17,.65,.23),(side*.13,.72,.34)],[.029,.022,.001],'Aged ivory',0)

def export(name,lod=0):
    data=bytearray(); count=0;groups={}
    for o in objects:
        proxy=None
        # Tiny closed tips collapse into two coincident opposing faces if reduced.
        # Keep their authored topology; only substantial surfaces need a LOD.
        if lod and len(o.data.polygons)>16:
            proxy=o.copy();proxy.data=o.data.copy();current.objects.link(proxy)
            bpy.context.view_layer.objects.active=proxy
            mod=proxy.modifiers.new('Distance detail reduction','DECIMATE');mod.ratio=.44 if lod==1 else .21
            bpy.ops.object.modifier_apply(modifier=mod.name)
        me=proxy.data if proxy else o.data;me.calc_loop_triangles();mat=o.data.materials[0]
        group=int(o['rig_group']);color=mat.diffuse_color[:3];engine=float(mat['engine_material'])
        normal_matrix=o.matrix_world.to_3x3().inverted().transposed()
        for tri in me.loop_triangles:
            a,b,c=[me.vertices[i].co for i in tri.vertices]
            if (b-a).cross(c-a).length_squared<1e-16: continue
            for li in tri.loops:
                p=o.matrix_world@me.vertices[me.loops[li].vertex_index].co
                n=(normal_matrix@me.corner_normals[li].vector).normalized()
                axis=max(range(3),key=lambda j:abs(n[j]));a,b=[(2,1),(0,2),(0,1)][axis]
                # Quiet vertex cavity shading adds depth, without baking illumination.
                data.extend(struct.pack('<I12f',group,*p,*n,*color,engine,p[a]*2.7,p[b]*2.7));count+=1
                groups[group]=groups.get(group,0)+1
        if proxy:
            mesh_data=proxy.data;bpy.data.objects.remove(proxy,do_unlink=True);bpy.data.meshes.remove(mesh_data)
    filename=name+('' if lod==0 else '-lod'+str(lod))
    (OUT/(filename+'.gvm')).write_bytes(b'GVM1'+struct.pack('<I',count)+data)
    print('CREATURE',filename,count//3,'triangles',flush=True)
    return dict(name=filename,triangles=count//3,groups=groups)

def arrange_source(kind):
    """Pose the editable Blender objects after exporting their local coordinates."""
    transforms=[Matrix.Identity(4) for _ in range(22)]
    head_y=2. if kind in [2,4] else .85 if kind==5 else 1.72
    transforms[1]=Matrix.Translation((0,head_y,0))
    for i,s in enumerate([-1,1]):
        shoulder=Vector((s*.34,1.4,0));elbow=Vector((s*.43,1.03,0));wrist=Vector((s*.42,.79,.12))
        hip=Vector((s*.15,.85,0));foot=Vector((s*.18,.085,0))
        bend=math.sqrt(.42**2-((foot-hip).length*.5)**2)
        knee=(hip+foot)*.5+Vector((0,0,bend))
        for g,a,b in [(2+i*3,shoulder,elbow),(3+i*3,elbow,wrist),(8+i*3,hip,knee),(9+i*3,knee,foot)]:
            d=b-a
            transforms[g]=Matrix.LocRotScale(a,Vector((0,1,0)).rotation_difference(d.normalized()),Vector((1,d.length,1)))
        transforms[4+i*3]=Matrix.LocRotScale(wrist,Vector((0,1,0)).rotation_difference((wrist-elbow).normalized()),Vector((1,1,1)))
        transforms[10+i*3]=Matrix.Translation(foot)
        transforms[14+i]=Matrix.Translation((s*.12,head_y-.06 if kind==4 else 1.48,-.08))
    for o in objects:
        o['runtime_local_matrix']=[v for row in o.matrix_world for v in row]
        o.matrix_world=transforms[o['rig_group']]@o.matrix_world

names=['ossuary-drudge','ribblade-skirmisher','cinder-skull','tithekeeper','gloamwing','grave-crawler','iron-penitent','plague-vessel','ash-cantor','bell-gargoyle','bone-shepherd','tithe-reaper']
reports=[]
for kind,name in enumerate(names):
    current=bpy.data.collections.new('%02d %s'%(kind,name));scene.collection.children.link(current);objects=[]
    skull(kind)
    if kind not in [2,4,5]:
        tube('Cervical vertebrae',[(0,1.43,-.06),(0,1.50,-.045),(0,1.56,-.04)],[.038,.030,.027],'Aged ivory',0)
        limbs(kind)
        if kind not in [8,10,11]:ribcage(kind)
        cloth(kind)
        if kind in [3,6]:armor(kind)
        weapon_prop(kind)
        if kind==7:
            ellipsoid('Swollen plague sac',(0,1.19,.025),(.28,.31,.19),'Plague skin',0,20,14)
            for i in range(9):
                a=i*2.4;y=1.+i*.037
                ellipsoid('Subdermal pustule',(math.sin(a)*.18,y,.17+math.cos(a)*.015),(.027,.031,.022),'Sickly light',0,10,6)
    if kind in [4,9]:
        for s,g in [(-1,14),(1,15)]:wing(kind,s,g)
    if kind==5:crawler()
    for lod in range(3):reports.append(export(name,lod))
    arrange_source(kind)
    current.hide_viewport=kind!=0;current.hide_render=kind!=0

# An actual Blender armature documents attachment frames. The runtime exports keep
# the pieces rigid, which preserves the game's detachable physics sections.
bpy.ops.object.armature_add()
rig=bpy.context.object;rig.name='Runtime segmented anatomy reference'
bpy.ops.object.mode_set(mode='EDIT');rig.data.edit_bones.remove(rig.data.edit_bones[0])
for name,a,b in [('torso',(0,.85,0),(0,1.45,0)),('head',(0,1.54,0),(0,1.94,0))]:
    bone=rig.data.edit_bones.new(name);bone.head=a;bone.tail=b
for s,side in [(-1,'L'),(1,'R')]:
    for name,a,b in [('upper_arm',(s*.34,1.4,0),(s*.43,1.03,0)),('forearm',(s*.43,1.03,0),(s*.42,.79,.12)),('thigh',(s*.15,.85,0),(s*.165,.47,.11)),('shin',(s*.165,.47,.11),(s*.18,.085,0))]:
        bone=rig.data.edit_bones.new(side+'_'+name);bone.head=a;bone.tail=b
bpy.ops.object.mode_set(mode='OBJECT');rig.hide_render=True;rig.hide_set(True)
scene['README']='Twelve original Gravewake creatures. Meshes store rig_group integer. Y-up, facing +Z. Rigid local segment coordinates are transformed by the engine shared anatomical pose; see src/enemy_assets.rs. Toggle named collections to inspect each creature.'
(OUT/'manifest.json').write_text(json.dumps(reports,indent=2))
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'assets/blender/gravewake-creatures.blend'))
print('CREATURE SOURCE SAVED',flush=True)
