struct Camera { vp:mat4x4<f32>, inverse_vp:mat4x4<f32>, eye:vec4<f32>, info:vec4<f32>, lights:array<vec4<f32>,6> };
@group(0) @binding(0) var<uniform> cam:Camera;
@group(0) @binding(1) var materials:texture_2d<f32>;
@group(0) @binding(2) var material_sampler:sampler;
@group(0) @binding(3) var weapon_color:texture_2d<f32>;
@group(0) @binding(4) var weapon_surface:texture_2d<f32>;
struct Out { @builtin(position) clip:vec4<f32>, @location(0) world:vec3<f32>, @location(1) normal:vec3<f32>, @location(2) color:vec3<f32>, @location(3) material:f32, @location(4) local:vec3<f32>, @location(5) wear_uv:vec2<f32> };
struct EnemyInstance {
 transforms:array<mat4x4<f32>,22>,
 normals:array<mat4x4<f32>,22>,
 params:vec4<f32>
};
@group(1) @binding(0) var<storage,read> enemy_instances:array<EnemyInstance>;
// Blender topology stays resident. Only the articulated rig crosses the CPU/GPU
// boundary each frame, preserving UVs and the exact detachable anatomy pose.
@vertex fn enemy_vs(@location(0) pos:vec3<f32>,@location(1) normal:vec3<f32>,
 @location(2) color:vec3<f32>,@location(3) material:f32,
 @location(4) local:vec3<f32>,@location(5) wear_uv:vec2<f32>,
 @builtin(instance_index) index:u32)->Out {
 let group=u32(local.z);
 let model=enemy_instances[index].transforms[group];
 let normal_model=enemy_instances[index].normals[group];
 let params=enemy_instances[index].params;
 let p=model*vec4(pos,1.);
 let world_n=(normal_model*vec4(normal,0.)).xyz;
 var o:Out;
 o.clip=cam.vp*p;o.world=p.xyz;
 o.normal=world_n/max(length(world_n),0.000001);
 o.color=select(color,vec3(2.3,1.8,1.),params.z>0. && material==2.);
 o.material=select(material,params.y,material==2.);
 o.local=vec3(local.xy,0.);o.wear_uv=wear_uv;
 if model[3][3]==0. {o.clip=vec4(2.,2.,2.,1.);o.normal=vec3(0.,1.,0.);}
 return o;
}
@vertex fn vs(@location(0) pos:vec3<f32>,@location(1) normal:vec3<f32>,@location(2) color:vec3<f32>,@location(3) material:f32,@location(4) local:vec3<f32>,@location(5) wear_uv:vec2<f32>)->Out {var o:Out;var p=pos;
 // Only foliage moves: trunks, stone and the first-person rig stay grounded.
 if material>4.5 && material<5.5 && cam.info.y<0.5 {
  let sway=sin(pos.x*0.31+pos.z*0.18+cam.info.x*0.65)*0.07+sin(pos.z*0.73-cam.info.x*1.15)*0.025;
  p.x+=sway*clamp(pos.y*0.10,0.,1.3)*cam.info.z;
  p.z+=sway*0.45*cam.info.z;
 }
 o.clip=cam.vp*vec4(p,1.);o.world=p;o.normal=normal;o.color=color;o.material=material;o.local=local;o.wear_uv=wear_uv;return o;}
// Shared full-detail sphere topology. Only per-shape transforms cross the CPU/GPU boundary.
@vertex fn sphere_vs(@location(0) pos:vec3<f32>,@location(1) normal:vec3<f32>,
 @location(6) c0:vec4<f32>,@location(7) c1:vec4<f32>,@location(8) c2:vec4<f32>,@location(9) c3:vec4<f32>,
 @location(10) center:vec4<f32>,@location(11) radius:vec4<f32>,@location(12) color:vec4<f32>)->Out {
 let model=mat4x4(c0,c1,c2,c3);let local=pos*radius.xyz+center.xyz;
 let raw=normal/max(radius.xyz,vec3(0.000001));let n=raw/max(length(raw),0.000001);
 let axes=abs(n);var uv=local.xy;
 if axes.y>axes.x && axes.y>axes.z {uv=local.xz;} else if axes.x>axes.z {uv=local.zy;}
 let p=model*vec4(local,1.);let world_n=(model*vec4(n,0.)).xyz;
 var o:Out;o.clip=cam.vp*p;o.world=p.xyz;o.normal=world_n/max(length(world_n),0.000001);
 o.color=color.xyz;o.material=center.w;o.local=vec3(uv,0.);o.wear_uv=uv*2.7;return o;
}
// Corpse sections stay resident; only their rigid-body transform (with the
// end-of-life shrink as a uniform scale) is uploaded each frame. Normals turn
// with the body but aren't rescaled, exactly as the CPU expansion did.
@vertex fn piece_vs(@location(0) pos:vec3<f32>,@location(1) normal:vec3<f32>,
 @location(2) color:vec3<f32>,@location(3) material:f32,
 @location(4) local:vec3<f32>,@location(5) wear_uv:vec2<f32>,
 @location(6) c0:vec4<f32>,@location(7) c1:vec4<f32>,@location(8) c2:vec4<f32>,@location(9) c3:vec4<f32>)->Out {
 let model=mat4x4(c0,c1,c2,c3);let p=model*vec4(pos,1.);
 var o:Out;o.clip=cam.vp*p;o.world=p.xyz;
 o.normal=(model*vec4(normal,0.)).xyz/max(length(c0.xyz),0.000001);
 o.color=color;o.material=material;o.local=local;o.wear_uv=wear_uv;return o;
}
fn hash(p:vec3<f32>)->f32 {return fract(sin(dot(p,vec3(12.9898,78.233,42.714)))*43758.5453);}
fn soil_noise(p:vec2<f32>)->f32 {
 let i=floor(p);let f=fract(p);let u=f*f*(3.-2.*f);
 return mix(mix(hash(vec3(i,7.)),hash(vec3(i+vec2(1.,0.),7.)),u.x),
  mix(hash(vec3(i+vec2(0.,1.),7.)),hash(vec3(i+vec2(1.,1.),7.)),u.x),u.y);
}
@fragment fn fs(in:Out)->@location(0) vec4<f32>{
 let status=in.material;
 let material=select(status,2.,status>=20. && status<=22.);
 if material<0. {let alpha=max(0.,1.-dot(in.local.xy,in.local.xy))*0.57;return vec4(0.,0.,0.,alpha);}
 if material>9.5 && material<10.5 {let r=length(in.local.xy);if r>1. {discard;}return vec4(in.color,pow(1.-r,3.)*0.35*in.local.z);}
 var n=normalize(in.normal);let distance=length(in.world-cam.eye.xyz);let grain=hash(floor(in.local*100.));var base=in.color;
 if material>0.5 && material<8. {
  var tile=vec2(0.,0.);var scale=0.42;
  if material>1.5 && material<2.5 {tile=vec2(1.,1.);scale=2.4;}
  if material>2.5 && material<3.5 {tile=vec2(2.,0.);scale=2.6;}
  if material>3.5 && material<4.5 {tile=vec2(1.,0.);scale=0.5;}
  if material>4.5 && material<5.5 {tile=vec2(2.,1.);scale=0.65;}
  if material>5.5 {tile=vec2(0.,1.);scale=0.5;}
  if material>6.5 {tile=vec2(2.,0.);scale=2.;}
  let uv=(tile+fract(in.local.xy*scale)*0.984+0.008)/vec2(3.,2.);
  let tex=textureSample(materials,material_sampler,uv).rgb;
  if material<1.5 {base=tex*0.75+in.color*0.12;}
  else if material<2.5 {
   // Weathered bone reflects the same dim light as the graveyard masonry.
   // Keep authored joint/species variation, with a muted ash-ivory palette.
   let pores=dot(tex,vec3(0.333));
   let ash=vec3(dot(in.color,vec3(0.2126,0.7152,0.0722)))*vec3(0.96,0.98,0.94);
   base=mix(in.color,ash,0.24)*(0.58+pores*0.22);
  }
  else if material<3.5 {let wear=dot(tex,vec3(0.333));base=mix(tex*0.67,in.color*(0.50+wear*1.7),select(0.1,0.92,in.color.r>0.35));}
  else if material<4.5 {base=tex*0.7*normalize(in.color+vec3(0.1));}
  else if material<5.5 {base=tex*vec3(0.28,0.5,0.37);}
  else if material<6.5 {
   // Continuous world-space soil: broad moss patches and fine gravel, without
   // the atlas seams that became visible across the newly opened landscape.
   let p=in.world.xz;
   let broad=soil_noise(p*0.23)*0.6+soil_noise(p*0.71+vec2(19.,7.))*0.4;
   let grit=soil_noise(p*8.3);
   let earth=mix(vec3(0.075,0.065,0.046),vec3(0.23,0.19,0.13),grit);
   let moss=mix(vec3(0.075,0.11,0.056),vec3(0.15,0.19,0.085),grit);
   base=mix(earth,moss,smoothstep(0.44,0.70,broad))*(0.8+broad*0.4);
  }else{base=in.color*(0.35+tex.r*3.);}
 }
 // Baked Blender materials: R roughness, G micro-height, B metalness.
 // Mesh UVs remain attached to each action part through aiming and reloads.
 var roughness=0.85;var metalness=0.;
 if material>10.5 && material<14.5 {
  let index=floor(material-11.+0.1);
  let tile=vec2(index%2.,1.-floor(index/2.));
  let uv=(tile+vec2(0.006)+fract(in.wear_uv)*0.988)/2.;
  let surface=textureSample(weapon_surface,material_sampler,uv).rgb;
  let albedo=textureSample(weapon_color,material_sampler,uv).rgb;
  roughness=clamp(surface.r,0.64,0.98);metalness=surface.b;
  let patinaTint=clamp(dot(in.color,vec3(0.333))*0.6+0.72,0.72,0.96);
  base=albedo*patinaTint;
  if material>13.5 && max(in.color.r,max(in.color.g,in.color.b))<0.02 {base=in.color;}
  let sx=dpdx(in.world);let sy=dpdy(in.world);
  let r1=cross(sy,n);let r2=cross(n,sx);let determinant=dot(sx,r1);
  let gradient=(r1*dpdx(surface.g)+r2*dpdy(surface.g))/max(abs(determinant),0.000001)*sign(determinant);
  n=normalize(n-clamp(gradient*0.006,vec3(-0.35),vec3(0.35)));
 }
 if in.color.r>1.5 && material<8. {base=in.color;}
 let fx=cam.info.z;
 let view=normalize(cam.eye.xyz-in.world);
 let stone=material>0.5 && material<1.5;
 let wet=select(0.,smoothstep(0.2,0.8,n.y)*(0.5+0.5*sin(in.world.x*0.43+sin(in.world.z*0.51))),stone)*fx;
 if wet>0.01 {
  n=normalize(n+vec3(sin(in.world.z*37.)*0.028,0.,cos(in.world.x*31.)*0.028)*wet);
  base*=1.-wet*0.14;
 }
 let moon=max(dot(n,normalize(vec3(-0.4,0.8,0.2))),0.);var lighting=vec3(0.095,0.15,0.18)+moon*vec3(0.12,0.19,0.22);
 for(var i=0;i<6;i++){let delta=cam.lights[i].xyz-in.world;let d=length(delta);let attenuation=9.0/(1.+d*d*0.55);let flicker=0.95+0.05*sin(cam.info.x*9.+f32(i)*3.);lighting+=vec3(1.8,0.68,0.17)*attenuation*(0.16+0.84*max(dot(n,normalize(delta)),0.))*flicker;}
 let bone=material>1.5 && material<2.5;
 let worn=material>10.5 && material<14.5;
 let closeFill=select(1.,select(0.62,0.45,worn),bone || worn);
 lighting+=vec3(0.21,0.16,0.10)*max(0.,1.-distance/5.)*closeFill;
 if cam.info.y>0.5 {lighting=vec3(0.40,0.43,0.48)+max(dot(n,normalize(vec3(-0.7,0.8,-0.9))),0.)*vec3(1.6,1.45,1.1);}
 // Near-camera muzzle illumination gives shots a brief warm response on
 // weapon surfaces and nearby enemies without flashing the entire frame.
 if cam.info.y<0.5 {
  lighting+=vec3(2.6,1.25,0.34)*cam.info.w*fx/(1.+distance*distance*2.5);
 }
 var color=base*lighting;
 if fx>0. && cam.info.y<0.5 {
  if stone {
   let moonHalf=normalize(view+normalize(vec3(-0.4,0.8,0.2)));
   color+=vec3(0.045,0.105,0.14)*pow(max(dot(n,moonHalf),0.),45.)*wet;
   for(var i=0;i<6;i++) {
    let d=cam.lights[i].xyz-in.world;let len=length(d);let half=normalize(view+normalize(d));
    let highlight=pow(max(dot(n,half),0.),mix(24.,95.,wet));
    color+=vec3(1.45,0.48,0.065)*highlight*wet/(1.+len*len*0.12);
   }
  }
  // A restrained cool rim separates bone silhouettes from the dark forest.
  if material>1.5 && material<2.5 {
   let rim=pow(1.-max(dot(n,view),0.),3.);
   color+=vec3(0.012,0.035,0.038)*rim*fx;
  }
 }
 if material>10.5 && material<14.5 {
  let v=normalize(cam.eye.xyz-in.world);
  let metal=material<12.5;let brass=material>11.5 && material<12.5;
  let exponent=mix(7.,65.,pow(1.-roughness,2.));
  let conductor=mix(vec3(0.28,0.30,0.27),vec3(0.55,0.34,0.12),select(0.,1.,brass));
  let f0=mix(vec3(0.035),conductor,metalness);
  let fresnel=f0+(vec3(1.)-f0)*pow(1.-max(dot(n,v),0.),5.);
  // Arena reflections come from the cool moon and nearby flames. The stronger
  // inspection key belongs only to the armory's separate item-preview camera.
  let studio=cam.info.y>0.5;
  let key=normalize(select(vec3(-0.4,0.8,0.2),vec3(-0.6,0.85,-0.7),studio));
  let rim=normalize(vec3(0.85,0.35,0.4));
  let h=normalize(v+key);let hr=normalize(v+rim);
  let spec=pow(max(dot(n,h),0.),exponent)*(1.-roughness)*0.85;
  let specRim=pow(max(dot(n,hr),0.),exponent)*(1.-roughness)*0.12;
  color=base*(lighting+vec3(0.012));
  let keyTint=select(vec3(0.18,0.28,0.34),vec3(0.8,0.65,0.43),studio);
  color+=fresnel*(keyTint*spec+vec3(0.18,0.25,0.29)*specRim);
  if cam.info.y<0.5 && metal {
   for(var i=0;i<6;i++) {
    let d=cam.lights[i].xyz-in.world;let half=normalize(v+normalize(d));
    color+=fresnel*vec3(1.1,0.43,0.13)*pow(max(dot(n,half),0.),exponent)*(1.-roughness)*fx/(1.+dot(d,d)*0.08);
   }
  }
 }
 if material>8. && material<9.5 {
  color=in.color;
  if cam.info.y<0.5 {
   let edge=pow(1.-abs(dot(n,view)),2.);
   let pulse=0.92+0.08*sin(cam.info.x*3.2+in.world.y*5.);
   color*=mix(1.,pulse+edge*0.22,fx);
  }
 }
 // Combat states are surface treatments, so frost and venom follow the
 // animated skeleton instead of floating as screen overlays.
 if status>=20. && status<=22. && fx>0. {
  let p=in.world;
  let edge=pow(1.-abs(dot(n,view)),2.);
  if status<20.5 {
   let ember=pow(max(0.,sin(p.y*32.-cam.info.x*9.+sin(p.x*23.+p.z*17.)*3.)),12.);
   color+=vec3(1.9,0.21,0.012)*(ember*0.40+edge*0.16)*fx;
  } else if status<21.5 {
   let vein=pow(1.-abs(sin(p.y*27.+p.x*16.+sin(p.z*23.))),14.);
   color=mix(color,color*vec3(0.55,0.86,1.18)+vec3(0.015,0.045,0.065),0.55*fx);
   color+=vec3(0.08,0.48,0.8)*(vein*0.4+edge*0.35)*fx;
  } else {
   let venom=0.5+0.5*sin(p.y*17.+sin(p.x*12.+p.z*9.)*2.-cam.info.x*2.);
   color+=vec3(0.09,0.42,0.015)*(venom*0.20+edge*0.35)*fx;
  }
 }
 let fog=1.-exp(-distance*mix(0.024,0.008,fx));if cam.info.y<0.5 {color=mix(color,vec3(0.014,0.035,0.046),clamp(fog,0.,0.95));}else{color=pow(color/(color+vec3(0.8)),vec3(1./2.2));}
 return vec4(color,1.);
}
struct SkyOut { @builtin(position) clip:vec4<f32>, @location(0) uv:vec2<f32> };
@vertex fn sky_vs(@builtin(vertex_index) i:u32)->SkyOut {let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));var o:SkyOut;o.clip=vec4(p[i],0.9999,1.);o.uv=p[i];return o;}
@fragment fn sky_fs(in:SkyOut)->@location(0) vec4<f32> {let w=cam.inverse_vp*vec4(in.uv,1.,1.);let ray=normalize(w.xyz/w.w-cam.eye.xyz);let height=clamp(ray.y,0.,1.);var color=mix(vec3(0.025,0.052,0.066),vec3(0.003,0.007,0.016),height);let p=ray.xz/(max(ray.y,0.10)+0.18);let clouds=sin(p.x*1.7+sin(p.y*2.1)+cam.info.x*0.011)*sin(p.y*2.5+sin(p.x*1.2))+0.4*sin(p.x*5.+sin(p.y*7.));color+=clouds*0.009*(1.-height);let moon=pow(max(dot(ray,normalize(vec3(-0.5,0.6,-0.8))),0.),1500.);color+=vec3(0.7,0.79,0.73)*moon;let stars=hash(floor(ray*700.));color+=select(0.,0.18,stars>0.9985)*height;return vec4(color,1.);}
