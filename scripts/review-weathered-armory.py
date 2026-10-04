"""Blender studio inspection renders; native engine captures remain the delivery gate."""
import bpy, math
from pathlib import Path
from mathutils import Vector, Matrix
ROOT=Path(__file__).resolve().parents[1]
bpy.ops.wm.open_mainfile(filepath=str(ROOT/'assets/blender/weathered-armory.blend'))
s=bpy.context.scene;s.render.engine='CYCLES';s.cycles.samples=24;s.cycles.use_denoising=True
s.render.resolution_x=960;s.render.resolution_y=660;s.render.resolution_percentage=100
s.world.use_nodes=True;s.world.node_tree.nodes['Background'].inputs[0].default_value=(.065,.075,.087,1);s.world.node_tree.nodes['Background'].inputs[1].default_value=.45
s.view_settings.view_transform='AgX';s.view_settings.look='AgX - Medium High Contrast'
colls=sorted([c for c in bpy.data.collections if c.name[:2].isdigit()],key=lambda c:c.name)
for i,c in enumerate(colls):
    for o in c.objects:o.location.x-=(i%4-1.5)*1.6;o.location.z-=(i//4)*2.05
cam_data=bpy.data.cameras.new('Inspection camera');cam=bpy.data.objects.new('Inspection camera',cam_data);s.collection.objects.link(cam);s.camera=cam;cam_data.type='ORTHO';cam_data.ortho_scale=1.9
for name,p,power,size,color in [('Warm softbox',(1.2,2,-.5),140,3,(1,.84,.65)),('Cool fill',(-2,.8,-1),100,2,(.63,.78,1)),('Rim',(.5,.4,2),180,2,(1,.68,.40))]:
    d=bpy.data.lights.new(name,'AREA');d.energy=power;d.shape='DISK';d.size=size;d.color=color
    o=bpy.data.objects.new(name,d);s.collection.objects.link(o);o.location=p;o.rotation_euler=(-o.location).to_track_quat('-Z','Y').to_euler()
out=ROOT/'captures/models/blender';out.mkdir(parents=True,exist_ok=True)
for i,c in enumerate(colls):
    for cc in colls:
        for o in cc.objects:o.hide_render=cc!=c
    target=Vector((0,-.02,-.12)) if i!=2 else Vector((0,.10,0))
    cam.location=target+Vector((2.3,1.1,-1.2))
    forward=(target-cam.location).normalized();right=forward.cross(Vector((0,1,0))).normalized();up=right.cross(forward)
    cam.rotation_euler=Matrix((right,up,-forward)).transposed().to_euler()
    cam_data.ortho_scale=1.35 if i in [0,2,3] else 1.95
    s.render.filepath=str(out/f'{i:02d}.png');bpy.ops.render.render(write_still=True)
