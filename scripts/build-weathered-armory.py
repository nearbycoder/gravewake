"""Blender 4.5+ authoring source. Run in background; creates editable source,
seven articulated game meshes, and a four-material color/surface atlas.
Coordinates intentionally match the engine: Y up, muzzle along -Z.
"""
import bpy, bmesh, math, struct, json, sys
REBAKE = "--rebake" in sys.argv
from pathlib import Path
from mathutils import Vector
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'assets/weapons'; OUT.mkdir(exist_ok=True)
bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete(use_global=False)
scene=bpy.context.scene
scene.render.engine='CYCLES'; scene.cycles.samples=8
scene.render.bake.margin=4
bpy.context.preferences.filepaths.save_version=0
materials=[]

def ramp(nodes,links,src,stops):
    n=nodes.new('ShaderNodeValToRGB')
    for el in list(n.color_ramp.elements)[2:]: n.color_ramp.elements.remove(el)
    for i,(pos,col) in enumerate(stops):
        el=n.color_ramp.elements[i] if i<2 else n.color_ramp.elements.new(pos)
        el.position=pos; el.color=(*col,1)
    links.new(src,n.inputs[0]); return n.outputs['Color']
def mathn(nodes,links,op,a,b=None):
    n=nodes.new('ShaderNodeMath'); n.operation=op
    for i,val in enumerate([a,b]):
        if val is None: continue
        if isinstance(val,(float,int)): n.inputs[i].default_value=val
        else: links.new(val,n.inputs[i])
    return n.outputs[0]
def mix(nodes,links,a,b,f):
    n=nodes.new('ShaderNodeMixRGB');
    for socket,val in [(n.inputs[0],f),(n.inputs[1],a),(n.inputs[2],b)]:
        if isinstance(val,tuple): socket.default_value=(*val,1)
        elif isinstance(val,(float,int)): socket.default_value=val
        else: links.new(val,socket)
    return n.outputs[0]
for index,name in enumerate(['Pitted iron','Oxidized brass','Splintered walnut','Oil-soaked leather']):
    mat=bpy.data.materials.new(name); mat.use_nodes=True; materials.append(mat)
    n=mat.node_tree.nodes; n.clear(); l=mat.node_tree.links
    out=n.new('ShaderNodeOutputMaterial'); shader=n.new('ShaderNodeBsdfPrincipled'); l.new(shader.outputs[0],out.inputs[0])
    uv=n.new('ShaderNodeTexCoord'); split=n.new('ShaderNodeSeparateXYZ'); l.new(uv.outputs['UV'],split.inputs[0])
    u=mathn(n,l,'MULTIPLY',split.outputs['X'],math.tau); v=mathn(n,l,'MULTIPLY',split.outputs['Y'],math.tau)
    torus=n.new('ShaderNodeCombineXYZ')
    for socket,source in zip(torus.inputs,[mathn(n,l,'COSINE',u),mathn(n,l,'SINE',u),mathn(n,l,'COSINE',v)]): l.new(source,socket)
    w=mathn(n,l,'SINE',v)
    def noise(scale,detail):
        t=n.new('ShaderNodeTexNoise'); t.noise_dimensions='4D'; t.inputs['Scale'].default_value=scale; t.inputs['Detail'].default_value=detail; t.inputs['Roughness'].default_value=.78
        l.new(torus.outputs[0],t.inputs['Vector']); l.new(w,t.inputs['W']); return t.outputs['Fac']
    broad=noise(2.1,4.); fine=noise(37.,2.); pits=noise(88.,1.)
    patch=ramp(n,l,broad,[(.57,(0,0,0)),(.74,(1,1,1))])
    pitmask=ramp(n,l,pits,[(.48,(1,1,1)),(.67,(.14,.14,.14))])
    if index==0:
        clean=mix(n,l,(.022,.034,.044),(.075,.100,.115),fine)
        rust=mix(n,l,(.036,.018,.010),(.13,.053,.018),fine)
        color=mix(n,l,clean,rust,patch)
        rough=mix(n,l,(.45,)*3,(.90,)*3,patch); metal=mix(n,l,(.80,)*3,(.06,)*3,patch)
    elif index==1:
        clean=mix(n,l,(.13,.078,.026),(.36,.24,.09),fine)
        patina=mix(n,l,(.032,.061,.042),(.09,.145,.097),fine)
        color=mix(n,l,clean,patina,patch)
        rough=mix(n,l,(.58,)*3,(.91,)*3,patch); metal=mix(n,l,(.75,)*3,(.15,)*3,patch)
    elif index==2:
        wave=n.new('ShaderNodeTexWave'); wave.wave_type='BANDS'; wave.bands_direction='X'; wave.inputs['Scale'].default_value=22.;wave.inputs['Distortion'].default_value=3.;wave.inputs['Detail'].default_value=4.
        l.new(uv.outputs['UV'],wave.inputs['Vector'])
        grain=wave.outputs['Fac']
        color=mix(n,l,(.047,.022,.011),(.073,.039,.021),grain)
        color=mix(n,l,color,(.19,.14,.085),mathn(n,l,'MULTIPLY',patch,.18))
        rough=mix(n,l,(.72,)*3,(.98,)*3,patch); metal=(0.,)*3; pitmask=mix(n,l,pitmask,grain,.3)
    else:
        color=mix(n,l,(.031,.018,.011),(.135,.078,.037),fine)
        color=mix(n,l,color,(.24,.19,.105),mathn(n,l,'MULTIPLY',patch,.35))
        rough=mix(n,l,(.76,)*3,(.98,)*3,patch);metal=(0.,)*3
    color=mix(n,l,(.012,.008,.006),color,pitmask)
    height=mathn(n,l,'MULTIPLY',mathn(n,l,'MULTIPLY',fine,pitmask),.25)
    l.new(color,shader.inputs['Base Color']);l.new(rough,shader.inputs['Roughness'])
    if isinstance(metal,tuple): shader.inputs['Metallic'].default_value=metal[0]
    else:l.new(metal,shader.inputs['Metallic'])
    bump=n.new('ShaderNodeBump'); bump.inputs['Strength'].default_value=.14;bump.inputs['Distance'].default_value=.005;l.new(height,bump.inputs['Height']);l.new(bump.outputs[0],shader.inputs['Normal'])
    mat['engine_material']=11+index
    mat['bake_color_socket']=color.node.name
    if REBAKE:
        # Bake each node material on a UV square to color and linear surface maps.
        bpy.ops.mesh.primitive_plane_add(size=2);plane=bpy.context.object;plane.name='BAKE_'+name;plane.data.materials.append(mat)
        emission=n.new('ShaderNodeEmission');l.new(emission.outputs[0],out.inputs['Surface'])
        for channel in ['color','surface']:
            image=bpy.data.images.new(f'{name}_{channel}',width=512,height=512,alpha=True)
            image.colorspace_settings.name='sRGB' if channel=='color' else 'Non-Color'
            tex=n.new('ShaderNodeTexImage');tex.image=image;n.active=tex
            if channel=='color': l.new(color,emission.inputs['Color'])
            else:
                combine=n.new('ShaderNodeCombineColor');combine.mode='RGB'
                l.new(rough,combine.inputs['Red']);l.new(height,combine.inputs['Green'])
                if isinstance(metal,tuple):combine.inputs['Blue'].default_value=0.
                else:l.new(metal,combine.inputs['Blue'])
                l.new(combine.outputs[0],emission.inputs['Color'])
            bpy.ops.object.bake(type='EMIT')
            image.filepath_raw=str(OUT/f'{index}-{channel}.png');image.file_format='PNG';image.save()
        l.new(shader.outputs[0],out.inputs[0]);bpy.data.objects.remove(plane,do_unlink=True)
if REBAKE:
    # Combine into a 2x2 atlas through Blender's image API. Surface remains linear.
    import numpy as np
    for channel in ['color','surface']:
        atlas=np.ones((1024,1024,4),dtype=np.float32)
        for index,mat in enumerate(materials):
            im=bpy.data.images[f'{mat.name}_{channel}'];px=np.asarray(im.pixels[:],dtype=np.float32).reshape(512,512,4)
            x=(index%2)*512;y=(index//2)*512;atlas[y:y+512,x:x+512]=px
        im=bpy.data.images.new('Weathered arsenal '+channel,width=1024,height=1024,alpha=True)
        im.colorspace_settings.name='sRGB' if channel=='color' else 'Non-Color'
        im.pixels.foreach_set(atlas.ravel());im.filepath_raw=str(OUT/f'weathered-{channel}.png');im.file_format='PNG';im.save()

else:
    for channel in ["color", "surface"]:
        image=bpy.data.images.load(str(OUT/f"weathered-{channel}.png"))
        image.colorspace_settings.name="sRGB" if channel=="color" else "Non-Color"

# Mesh authoring helpers. Y up; muzzle -Z. Every object is editable in Blender.
# Seven assemblies share the existing baked material atlas. Surface complexity comes
# from fitted forms, recessed seams and restrained hardware, not inflated box parts.
current=None; objects=[]; model_stats=[]; authoring_collections=[]

def finish(obj,name,material,group=0,bevel=0,smooth=True):
    obj.name=name;obj.data.materials.append(materials[material]);obj['rig_group']=group
    bpy.ops.object.select_all(action='DESELECT');obj.select_set(True);bpy.context.view_layer.objects.active=obj
    bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
    if bevel:
        mod=obj.modifiers.new('Forged edge radii','BEVEL');mod.width=bevel;mod.segments=2;mod.limit_method='ANGLE';mod.angle_limit=.35
        bpy.ops.object.modifier_apply(modifier=mod.name)
    if smooth:
        for p in obj.data.polygons:p.use_smooth=True
        # Split genuine creases while keeping cylindrical and bevel bands continuous.
        edge_faces={}
        for p in obj.data.polygons:
            for e in p.edge_keys:edge_faces.setdefault(tuple(sorted(e)),[]).append(p)
        for e in obj.data.edges:
            faces=edge_faces.get(tuple(sorted(e.vertices)),[])
            e.use_edge_sharp=len(faces)==2 and faces[0].normal.dot(faces[1].normal)<math.cos(.72)
        wn=obj.modifiers.new('Area weighted corner normals','WEIGHTED_NORMAL');wn.keep_sharp=True;wn.weight=40
        bpy.ops.object.modifier_apply(modifier=wn.name)
    uv=obj.data.uv_layers.new(name='Weathering UV')
    for poly in obj.data.polygons:
        axis=max(range(3),key=lambda i:abs(poly.normal[i]));pair=[(2,1),(0,2),(0,1)][axis]
        for loop in poly.loop_indices:
            p=obj.data.vertices[obj.data.loops[loop].vertex_index].co
            # Long grain follows the authored length of stock and grip pieces.
            uv.data[loop].uv=(p[pair[0]]*2.7+.173,p[pair[1]]*2.7+.271)
    for coll in list(obj.users_collection):coll.objects.unlink(obj)
    current.objects.link(obj);objects.append(obj);return obj

def mesh(name,verts,faces,mat=0,group=0,bevel=0,smooth=True):
    me=bpy.data.meshes.new(name);me.from_pydata(verts,[],faces);me.update()
    bm=bmesh.new();bm.from_mesh(me);bmesh.ops.recalc_face_normals(bm,faces=bm.faces);bm.to_mesh(me);bm.free();me.update()
    o=bpy.data.objects.new(name,me);bpy.context.collection.objects.link(o)
    return finish(o,name,mat,group,bevel,smooth)
def box(name,p,size,mat=0,group=0,bevel=.004):
    bpy.ops.mesh.primitive_cube_add(size=1,location=p);o=bpy.context.object;o.scale=size
    return finish(o,name,mat,group,bevel)
def profile(name,outline,width,mat=0,group=0,bevel=.006,x=0):
    # Outline expressed as (z,y). Broad side planes stay flat; bevels carry light.
    N=len(outline);verts=[(x+s*width*.5,y,z) for s in [-1,1] for z,y in outline]
    # The input is counterclockwise in z/y coordinates (normal toward -X).
    faces=[tuple(range(N)),tuple(reversed(range(N,2*N)))]+[(i+N,(i+1)%N+N,(i+1)%N,i) for i in range(N)]
    return mesh(name,verts,faces,mat,group,bevel)
def lathe(name,p0,p1,sections,mat=0,group=0,count=24):
    a=Vector(p0);b=Vector(p1);axis=(b-a).normalized()
    u=axis.cross(Vector((0,1,0)) if abs(axis.y)<.9 else Vector((1,0,0))).normalized();v=axis.cross(u)
    verts=[a+axis*z+(u*math.cos(i*math.tau/count)+v*math.sin(i*math.tau/count))*r for z,r in sections for i in range(count)]
    faces=[]
    for k in range(len(sections)-1):
        for i in range(count):faces.append((k*count+i,k*count+(i+1)%count,(k+1)*count+(i+1)%count,(k+1)*count+i))
    return mesh(name,verts,faces,mat,group)
def tube(name,p0,p1,r,mat=0,group=0,inner=0,count=20):
    L=(Vector(p1)-Vector(p0)).length
    sections=[(0,0),(0,r),(L,r),(L,inner),(max(.001,L-.16),inner)] if inner else [(0,0),(0,r),(L,r),(L,0)]
    return lathe(name,p0,p1,sections,mat,group,count)
def barrel(name,x,y,start,end,r,group=0):
    L=start-end
    sections=[(0,r*.80 if group==1 else 0),(0,r*1.20),(.018,r*1.20),(.04,r*1.08),(L*.65,r*.92),(L-.026,r*.88),(L-.008,r*.94),(L,r*.89),(L,r*.65),(L-.008,r*.63),(L-.18,r*.63)]
    sections += [(.18,r*.80),(0,r*.80)] if group==1 else [(L-.18,0)]
    o=lathe(name,(x,y,start),(x,y,end),sections,0,group,32)
    o.data.materials.append(materials[3])
    for p in o.data.polygons:
        if p.index>=8*32:p.material_index=1
    return o
def loft(name,rings,mat=2,group=0,count=16):
    verts=[]
    for p,rx,ry in rings:
        for i in range(count):
            a=i*math.tau/count;verts.append((p[0]+math.cos(a)*rx,p[1]+math.sin(a)*ry,p[2]))
    faces=[]
    for k in range(len(rings)-1):
        for i in range(count):faces.append((k*count+i,k*count+(i+1)%count,(k+1)*count+(i+1)%count,(k+1)*count+i))
    faces.extend([tuple(reversed(range(count))),tuple(range((len(rings)-1)*count,len(rings)*count))])
    return mesh(name,verts,faces,mat,group,.0015)
def wire(name,points,r=.006,mat=0,group=0,closed=False):
    # Single smooth swept curve; seamless bends, no stack of intersecting rods.
    curve=bpy.data.curves.new(name,'CURVE');curve.dimensions='3D';curve.resolution_u=3;curve.bevel_depth=r;curve.bevel_resolution=1;curve.resolution_u=3
    spline=curve.splines.new('BEZIER');spline.bezier_points.add(len(points)-1)
    for p,co in zip(spline.bezier_points,points):p.co=co;p.handle_left_type='AUTO';p.handle_right_type='AUTO'
    spline.use_cyclic_u=closed
    o=bpy.data.objects.new(name,curve);bpy.context.collection.objects.link(o)
    bpy.ops.object.select_all(action='DESELECT');o.select_set(True);bpy.context.view_layer.objects.active=o;bpy.ops.object.convert(target='MESH')
    return finish(bpy.context.object,name,mat,group)
def screw(name,x,y,z,group=0,r=.008):
    sign=1 if x>=0 else -1
    tube(name,(x-sign*.002,y,z),(x+sign*.002,y,z),r,0,group,count=16)
    # Recessed screwdriver slot makes these read as hardware, not gold rivets.
    box(name+' slot',(x+sign*.0024,y,z),(.001,.002,r*1.35),3,group,.0005)
def guard(z=.03,y=-.073,group=0):
    wire('Continuous forged trigger bow',[(0,y,z+.10),(0,y-.085,z+.067),(0,y-.095,z-.018),(0,y-.055,z-.08),(0,y+.018,z-.074)],.008,0,group)
    wire('Curved trigger',[(0,y+.008,z+.031),(0,y-.023,z+.021),(0,y-.055,z+.035)],.008,0)
def grip():
    outline=[(.11,.04),(.19,.044),(.204,-.052),(.264,-.252),(.23,-.292),(.135,-.276),(.09,-.204),(.107,-.107),(.075,-.04)]
    profile('Swept steel grip frame',outline,.082,0,bevel=.012)
    panel=[(.129,-.045),(.181,-.035),(.195,-.104),(.242,-.248),(.218,-.270),(.145,-.250),(.12,-.198),(.135,-.113)]
    for s in [-1,1]:
        profile('Fitted walnut grip scale',panel,.014,2,bevel=.009,x=s*.044)
        for y,z in [(-.078,.155),(-.225,.195)]:screw('Grip screw',s*.055,y,z)
        for j in range(6):
            z=.142+j*.010;wire('Fine hand cut grip checkering',[(s*.052,-.13,z),(s*.053,-.19,z+.03)],.0011,3)
    box('Steel heel cap',(0,-.278,.194),(.089,.016,.091),0,bevel=.005)
def stock(short=False):
    end=.52 if short else .68
    loft('Sculpted walnut wrist comb and butt', [((0,-.018,.105),.056,.055),((0,-.03,.19),.056,.063),((0,-.07,.25),.045,.059),((0,-.091,.31),.055,.079),((0,-.081,end-.19),.068,.104),((0,-.08,end-.055),.078,.137),((0,-.09,end),.076,.143)],2)
    loft('Contoured steel butt plate',[((0,-.09,end-.004),.077,.144),((0,-.09,end+.012),.078,.144),((0,-.09,end+.02),.073,.138)],0)
    for s in [-1,1]:
        for y in [.002,-.18]:screw('Butt plate retaining pin',s*.076,y,end-.007,r=.006)
        # Small fitted comb insert rather than visually dominant repair bands.
        profile('Wrist checkered inset',[(.225,-.025),(.305,-.038),(.35,-.112),(.272,-.155),(.223,-.111)],.006,3,bevel=.005,x=s*.048)
    tube('Rear sling eye',(0,-.21,end-.12),(0,-.236,end-.12),.010,0)
def receiver(long=True):
    profile('Machined receiver body',[(-.115,-.058),(-.13,.041),(-.093,.109),(.098,.117),(.155,.084),(.154,-.035),(.111,-.069),(-.018,-.073)],.166 if long else .143,0,bevel=.012)
    for s in [-1,1]:
        x=s*(.085 if long else .074)
        profile('Inset action cover',[(-.098,-.035),(-.094,.054),(-.061,.077),(.106,.073),(.128,.042),(.118,-.035),(.057,-.053),(-.063,-.05)],.007,0,bevel=.0035,x=x)
        for y,z in [(.022,-.064),(.005,.09)]:screw('Action cover screw',x+s*.006,y,z)
def hammer(x=0,z=.147,y=.10):
    profile('Spurred hammer',[(z-.024,y-.031),(z+.014,y-.021),(z+.048,y+.038),(z+.035,y+.066),(z+.014,y+.06),(z+.008,y+.022)],.025,0,4,.003,x)
def collection(name):
    global current,objects
    current=bpy.data.collections.new(name);scene.collection.children.link(current);objects=[];authoring_collections.append(current)
def export(name):
    payload=bytearray();count=0;bounds=[];groups=set()
    for o in objects:
        me=o.data;me.calc_loop_triangles();uv=me.uv_layers.active.data;group=o['rig_group'];groups.add(group)
        for tri in me.loop_triangles:
            material=int(o.data.materials[tri.material_index]['engine_material'])
            a,b,c=(me.vertices[me.loops[li].vertex_index].co for li in tri.loops)
            # A lathe pole collapses a quad edge to a point; omit its zero-area half.
            if (b-a).cross(c-a).length_squared < 1e-18: continue
            for li in tri.loops:
                loop=me.loops[li];v=me.vertices[loop.vertex_index];p=o.matrix_world@v.co
                normal=me.corner_normals[li].vector
                if normal.length_squared < .5: normal=tri.normal
                norm=(o.matrix_world.to_3x3()@normal).normalized();tex=uv[li].uv
                payload.extend(struct.pack('<I9f',group,*p,*norm,material,*tex));count+=1
    (OUT/f'{name}.dvm').write_bytes(b'DVM1'+struct.pack('<I',count)+payload)
    model_stats.append({'name':name,'triangles':count//3,'objects':len(objects),'groups':sorted(groups)})
    print('EXPORTED',name,count,'vertices',flush=True)

collection('01 - Iron toggle pistol')
grip()
profile('Slim forged pistol receiver',[(-.159,-.035),(-.159,.062),(-.109,.089),(.125,.079),(.154,.041),(.13,-.046),(.055,-.053)],.137,0,bevel=.009)
barrel('Tapered pistol barrel',0,.08,-.125,-.52,.037)
# Toggle mechanism sits above the receiver, with visible pin and separated link plates.
profile('Upper breech block',[(-.102,.084),(-.102,.142),(.046,.142),(.096,.111),(.088,.078)],.091,0,2,.005)
for s in [-1,1]:
    profile('Toggle link side',[(-.015,.126),(.020,.150),(.098,.157),(.118,.143),(.087,.113)],.018,0,2,.004,x=s*.054)
    tube('Knurled toggle knob',(s*.060,.149,.08),(s*.084,.149,.08),.022,0,2,count=20)
    screw('Receiver pin',s*.071,.004,.099,r=.008)
    for j in range(4):box('Toggle knob knurl',(s*.083,.135+j*.008,.08),(.002,.003,.031),3,2,.0005)
profile('Detachable single stack magazine',[(.138,-.100),(.185,-.087),(.241,-.259),(.210,-.278),(.152,-.255)],.052,0,3,.003)
box('Magazine toe',(0,-.280,.196),(.094,.018,.112),0,3,.005)
guard(z=.020,y=-.061)
box('Rear notch sight',(0,.16,.112),(.067,.017,.020),0,bevel=.003)
box('Rear notch',(0,.166,.11),(.023,.016,.026),3,bevel=.001)
profile('Front blade sight',[(-.49,.113),(-.47,.142),(-.453,.142),(-.458,.111)],.012,0,bevel=.0015)
export('iron-pistol')

collection('02 - Mournhollow double barrel')
stock();receiver();guard();hammer(-.035);hammer(.035)
for s in [-1,1]:
    barrel('Cold blued shotgun tube',s*.060,.074,-.063,-.79,.057,1)
    # Barrel shoes carry the chamber and hinge; no ornate bands floating on a tube.
    lathe('Hollow chamber shoulder',(s*.060,.074,-.064),(s*.060,.074,-.194),[(0,.047),(0,.067),(.014,.067),(.13,.059),(.13,.047),(0,.047)],0,1,28)
    screw('Break action hinge',s*.089,-.035,-.062,r=.012)
loft('Rounded walnut splinter fore-end',[((0,-.040,-.10),.062,.022),((0,-.047,-.16),.087,.036),((0,-.044,-.32),.080,.039),((0,-.036,-.48),.059,.029),((0,-.027,-.52),.03,.018)],2,1)
box('Fine soldered barrel rib',(0,.122,-.415),(.019,.014,.71),0,1,.003)
tube('Front brass bead',(0,.130,-.742),(0,.144,-.742),.008,1,1,count=16)
wire('Top opening lever',[(0,.136,.091),(.005,.145,.133),(.055,.145,.174)],.012,0)
for s in [-1,1]:
    for i in range(6):
        wire('Fore-end grip checkering',[(s*.077,-.043,-.19-i*.027),(s*.074,-.065,-.18-i*.027)],.0011,3,1)
    wire('Understated receiver inlay',[(s*.090,.04,-.031),(s*.09,.052,.020),(s*.09,.035,.058)],.0014,1)
export('double-barrel')

collection('03 - Ossuary cleaver')
# A curved full tang, oval wooden scales, and a deliberately asymmetrical blade.
profile('Full tang handle',[(.026,-.325),(.035,-.290),(.026,-.03),(-.025,.03),(-.037,-.031),(-.038,-.25),(-.014,-.326)],.035,0,bevel=.004)
for s in [-1,1]:
    profile('Walnut handle scale',[(.022,-.291),(.026,-.25),(.017,-.041),(-.023,-.035),(-.029,-.248),(-.012,-.296)],.019,2,bevel=.009,x=s*.025)
    for y in [-.075,-.252]:screw('Peened tang pin',s*.037,y,-.001,r=.009)
# Blade cross-sections taper to a real sharp cutting edge; chipped notches remain intentional.
outline=[(-.09,.035),(.10,.035),(.143,.40),(.136,.525),(.091,.568),(-.076,.58),(-.108,.551),(-.102,.481),(-.118,.463),(-.108,.43),(-.131,.36),(-.12,.333),(-.125,.208)]
N=len(outline);inner=[(x*.83,y*.98+.008) for x,y in outline]
verts=[(x,y,z) for z in [-.0015,.0015] for x,y in outline]+[(x,y,z) for z in [-.016,.016] for x,y in inner]
faces=[]
for i in range(N):
    j=(i+1)%N;faces.extend([(i,j,j+N,i+N),(i+2*N,j+2*N,j,i),(i+N,j+N,j+3*N,i+3*N)])
faces.extend([tuple(reversed(range(2*N,3*N))),tuple(range(3*N,4*N))])
mesh('Forged blade with honed bevel',verts,faces,0,bevel=.0008)
# A recessed forged fuller / spine rib defines the broad face.
for s in [-1,1]:
    wire('Shallow blade spine detail',[(.082,.11,s*.017),(.100,.35,s*.017),(.090,.49,s*.017)],.002,3)
tube('Oval guard',(-.105,.006,0),(.108,.006,0),.018,0,count=20)
export('cleaver')

collection('04 - Grave revolver')
grip();guard(z=.012,y=-.080)
profile('Open revolver lower frame',[(-.155,-.098),(-.162,-.051),(.115,-.045),(.16,.028),(.165,.074),(.205,.048),(.182,-.067),(.08,-.115),(-.086,-.121)],.109,0,bevel=.009)
profile('Revolver top strap',[(-.17,.112),(-.142,.152),(.098,.157),(.17,.103),(.162,.073),(.097,.113),(-.118,.111)],.084,0,bevel=.006)
barrel('Heavy revolver barrel',0,.087,-.144,-.52,.042)
profile('Ejector rod shroud',[(-.17,.022),(-.17,-.025),(-.465,-.023),(-.48,.014)],.048,0,bevel=.009)
# Cylinder group swings about a crane on reload, rotates after the shot.
lathe('Fluted six chamber cylinder',(0,.027,.101),(0,.027,-.11),[(0,0),(0,.098),(.012,.110),(.184,.110),(.209,.097),(.209,0)],0,5,36)
for i in range(6):
    a=i*math.tau/6;xx=math.cos(a)*.070;yy=.027+math.sin(a)*.070
    tube('Cylinder chamber bore',(xx,yy,-.106),(xx,yy,-.114),.022,3,5,inner=.017,count=16)
    # Long shallow flutes are inset darker steel, with thin bright shoulders.
    rr=.108; x=math.cos(a)*rr;y=.027+math.sin(a)*rr
    wire('Cylinder flute',[(x*.94,.027+(y-.027)*.94,-.074),(x,y,-.056),(x,y,.060),(x*.94,.027+(y-.027)*.94,.076)],.0035,3,5)
    tube('Chamber case rim',(xx,yy,.101),(xx,yy,.104),.022,1,5,count=16)
    tube('Primer',(xx,yy,.104),(xx,yy,.105),.008,0,5,count=12)
wire('Cylinder crane',[(0,-.074,-.081),(-.05,-.079,-.085),(-.10,-.041,-.02)],.012,0,5)
hammer(0,.145,.116)
profile('Front revolver ramp',[(-.475,.128),(-.460,.167),(-.436,.167),(-.43,.128)],.015,0,bevel=.002)
for s in [-1,1]:screw('Frame cross pin',s*.061,-.052,.072,r=.009)
export('grave-revolver')

collection('05 - Slugbreaker pump shotgun')
stock();receiver();guard();hammer()
barrel('Single heavy slug barrel',0,.079,-.09,-.79,.055)
tube('Tubular magazine',(0,-.029,-.10),(0,-.029,-.727),.034,0,count=24)
lathe('Magazine end cap',(0,-.029,-.707),(0,-.029,-.73),[(0,.035),(.006,.040),(.020,.039),(.024,0)],0)
loft('Walnut sliding pump',[((0,-.033,-.22),.057,.054),((0,-.031,-.25),.065,.058),((0,-.033,-.43),.063,.056),((0,-.031,-.46),.054,.045)],2,6)
for i in range(7):
    z=-.254-i*.025
    loft('Recessed pump finger groove',[((0,-.032,z+.002),.065,.057),((0,-.032,z-.002),.065,.057)],3,6)
for s in [-1,1]:box('Pump action bar',(s*.055,-.042,-.155),(.009,.015,.205),0,6,.002)
profile('Right ejection port',[(-.055,.033),(-.056,.078),(.045,.080),(.073,.062),(.063,.031)],.005,3,bevel=.006,x=.087)
profile('Breech bolt face',[(-.051,.038),(-.051,.07),(.045,.072),(.062,.059),(.056,.037)],.006,0,2,.004,x=.091)
box('Bead sight base',(0,.131,-.738),(.026,.018,.028),0,bevel=.003)
tube('Brass sight bead',(0,.140,-.738),(0,.149,-.738),.008,1,count=16)
export('slug-pump')

collection('06 - Churchyard repeating rifle')
stock();receiver();guard(z=.035,y=-.075);hammer()
barrel('Stepped repeater barrel',0,.078,-.087,-.79,.036)
tube('Under barrel magazine',(0,-.019,-.10),(0,-.019,-.738),.025,0,count=24)
loft('Tapered walnut rifle fore-end',[((0,-.008,-.108),.061,.045),((0,-.011,-.17),.059,.044),((0,-.005,-.38),.050,.039),((0,-.002,-.49),.044,.031),((0,-.003,-.515),.037,.028)],2)
loft('Steel fore-end nose cap',[((0,-.003,-.498),.045,.032),((0,-.003,-.519),.042,.032)],0)
# Side loading gate and separate breech are unmistakable lever-action features.
profile('Side loading gate recess',[(-.049,-.006),(-.049,.031),(.065,.031),(.08,.013),(.067,-.006)],.005,3,bevel=.004,x=.090)
profile('Spring loading gate',[(-.043,-.002),(-.043,.027),(.063,.027),(.072,.012),(.062,-.002)],.005,0,bevel=.004,x=.093)
wire('Oval operating lever',[(0,-.079,.148),(0,-.169,.208),(0,-.206,.131),(0,-.204,-.006),(0,-.146,-.037),(0,-.088,-.022)],.011,0,7,True)
profile('Sliding breech bolt',[(-.080,.088),(-.081,.13),(.097,.13),(.125,.103),(.118,.087)],.07,0,2,.005)
profile('Buckhorn rear sight',[(-.304,.115),(-.289,.151),(-.269,.150),(-.261,.113)],.069,0,bevel=.002)
box('Rear sight notch',(0,.148,-.28),(.022,.019,.026),3,bevel=.001)
profile('Front sight blade',[(-.756,.110),(-.746,.136),(-.725,.135),(-.724,.105)],.011,0,bevel=.001)
export('repeater')

collection('07 - Pilgrim longrifle')
stock();receiver();guard();hammer()
barrel('Long rifled barrel',0,.078,-.099,-1.18,.032)
loft('Long tapered walnut fore-end',[((0,-.012,-.11),.061,.046),((0,-.005,-.23),.055,.046),((0,.004,-.50),.043,.034),((0,.009,-.75),.035,.029),((0,.012,-.90),.031,.025)],2)
for z,r in [(-.50,.045),(-.886,.035)]:
    loft('Steel retaining band',[((0,.005,z+.01),r,r*.83),((0,.005,z-.01),r,r*.83)],0)
profile('Bolt body',[(-.089,.083),(-.09,.127),(.14,.128),(.164,.112),(.156,.083)],.06,0,2,.006)
wire('Bent bolt handle',[(.017,.109,.10),(.102,.087,.113),(.127,.027,.129)],.011,0,2)
bpy.ops.mesh.primitive_uv_sphere_add(segments=16,ring_count=8,radius=.024,location=(.127,.027,.129));finish(bpy.context.object,'Rounded bolt knob',0,2)
# Scope has mount separation, tapered bells, recessed glass, and physical turrets.
for z in [-.06,-.38]:
    profile('Dovetail scope foot',[(z-.024,.10),(z-.024,.182),(z+.024,.182),(z+.024,.10)],.045,0,bevel=.004)
    tube('Scope retaining ring',(0,.214,z+.012),(0,.214,z-.012),.052,0,count=24)
lathe('Vintage telescopic sight',(0,.214,.08),(0,.214,-.62),[(0,.048),(.02,.048),(.08,.034),(.45,.034),(.54,.057),(.62,.059),(.64,.055),(.64,.047),(.614,.047)],0,count=32)
tube('Recessed front optic',(0,.214,-.528),(0,.214,-.529),.046,3,count=24)
tube('Recessed ocular',(0,.214,.071),(0,.214,.072),.039,3,count=24)
tube('Elevation dial',(0,.24,-.17),(0,.277,-.17),.025,0,count=20)
tube('Windage dial',(.025,.214,-.17),(.064,.214,-.17),.021,0,count=20)
profile('Longrifle front sight',[(-1.15,.107),(-1.135,.135),(-1.118,.135),(-1.113,.104)],.012,0,bevel=.001)
export('longrifle')

# Source laid out as a workshop sheet; export above always uses local engine space.
for i,coll in enumerate(authoring_collections):
    for o in coll.objects:
        o.location.x+=(i%4-1.5)*1.6;o.location.z+=(i//4)*2.05
scene.world.color=(.12,.12,.12)
for im in bpy.data.images:
    if im.type=='IMAGE' and im.size[0]>0:im.pack()
scene['notes']='Seven fitted low-poly weapons. Y up / muzzle -Z. Groups: 0 frame,1 break barrels,2 bolt/toggle,3 magazine,4 hammer,5 cylinder,6 pump,7 lever. Atlas UVs remain attached through animation. Smooth split corner normals exported per vertex.'
# Set the saved viewport to material inspection, with every collection named.
for screen in bpy.data.screens:
    for area in screen.areas:
        if area.type=='VIEW_3D':
            area.spaces.active.shading.type='MATERIAL';area.spaces.active.clip_end=100
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'assets/blender/weathered-armory.blend'))
(OUT/'manifest.json').write_text(json.dumps({'source':'../blender/weathered-armory.blend','blender':bpy.app.version_string,'materials':['pitted iron','oxidized brass','splintered walnut','oil-soaked leather'],'mesh_format':'DVM1: count u32, then (rig u32, position xyz, normal xyz, material f32, UV xy) little-endian','models':model_stats,'atlas_size':[1024,1024],'groups':{'0':'fixed frame','1':'break action barrels','2':'bolt or toggle','3':'detachable magazine','4':'hammer','5':'revolver cylinder','6':'pump slide','7':'operating lever'}},indent=2))
print('WEATHERED ARMORY EXPORT COMPLETE',flush=True)
