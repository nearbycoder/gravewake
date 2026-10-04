// Hollowlight composite. HDR scene and depth are sampled before egui draws the UI.
struct Camera { vp:mat4x4<f32>, inverse_vp:mat4x4<f32>, eye:vec4<f32>, info:vec4<f32>, lights:array<vec4<f32>,6> };
@group(0) @binding(0) var scene:texture_2d<f32>;
@group(0) @binding(1) var scene_sampler:sampler;
@group(0) @binding(2) var depth:texture_depth_2d;
@group(0) @binding(3) var<uniform> cam:Camera;
struct Out {@builtin(position) p:vec4<f32>,@location(0) uv:vec2<f32>};
@vertex fn vs(@builtin(vertex_index) i:u32)->Out {
 let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));var o:Out;
 o.p=vec4(p[i],0.,1.);o.uv=vec2(p[i].x*0.5+0.5,0.5-p[i].y*0.5);return o;
}
fn read_depth(uv:vec2<f32>)->f32 {
 let size=vec2<i32>(textureDimensions(depth));
 return textureLoad(depth,clamp(vec2<i32>(uv*vec2<f32>(size)),vec2(0),size-vec2(1)),0);
}
fn position(uv:vec2<f32>,d:f32)->vec3<f32> {
 let p=cam.inverse_vp*vec4(uv.x*2.-1.,1.-uv.y*2.,d,1.);
 return p.xyz/p.w;
}
fn luminance(c:vec3<f32>)->f32 {return dot(c,vec3(0.2126,0.7152,0.0722));}
fn filmic(x:vec3<f32>)->vec3<f32> {
 return clamp((x*(2.51*x+vec3(0.03)))/(x*(2.43*x+vec3(0.59))+vec3(0.14)),vec3(0.),vec3(1.));
}
@fragment fn fs(in:Out)->@location(0) vec4<f32> {
 let uv=in.uv;let texel=1./vec2<f32>(textureDimensions(scene));
 let original=textureSample(scene,scene_sampler,uv).rgb;
 let z=read_depth(uv);let p=position(uv,z);let delta=p-cam.eye.xyz;
 let distance=min(length(delta),85.);let ray=normalize(delta);
 let dx=dpdx(p);let dy=dpdy(p);
 var normal=normalize(cross(dx,dy));if dot(normal,-ray)<0. {normal=-normal;}
 let amount=cam.info.z;
 if amount<0.001 {
  var bloom=vec3(0.);
  for(var y=-2;y<=2;y++){for(var x=-2;x<=2;x++){
   bloom+=max(textureSampleLevel(scene,scene_sampler,uv+vec2(f32(x),f32(y))*texel*3.,0.).rgb-vec3(0.9),vec3(0.));
  }}
  var c=original+bloom/25.*0.38;
  c*=1.-0.5*pow(length((uv-0.5)*1.35),1.5);
  return vec4(c/(c+vec3(0.84)),1.);
 }
 var color=original;
 // Depth-reconstructed local occlusion: neighbouring geometry darkens creases,
 // without a full-screen outline around silhouettes or the first-person weapon.
 var occlusion=0.;
 let directions=array<vec2<f32>,8>(vec2(1.,0.),vec2(-1.,0.),vec2(0.,1.),vec2(0.,-1.),vec2(0.707,0.707),vec2(-0.707,0.707),vec2(0.707,-0.707),vec2(-0.707,-0.707));
 let radius=clamp(125./max(distance,1.),3.,24.);
 if z<0.99999 && distance>1.2 {
  for(var i=0;i<8;i++) {
   let qUV=clamp(uv+directions[i]*texel*radius,vec2(0.001),vec2(0.999));
   let q=position(qUV,read_depth(qUV));let v=q-p;let len=length(v);
   let facing=max(dot(normal,v/max(len,0.0001))-0.16,0.);
   occlusion+=facing*(1.-smoothstep(0.12,1.1,len));
  }
 }
 color*=1.-clamp(occlusion/8.*1.6,0.,0.36)*amount;
 // Integrate height-dependent mist along the visible ray. The depth endpoint
 // keeps fog behind nearby weapons and in front of distant silhouettes.
 var optical=0.;var scattered=vec3(0.);
 let step=distance/8.;
 for(var i=0;i<8;i++) {
  let t=(f32(i)+0.5)*step;let w=cam.eye.xyz+ray*t;
  let drift=cam.info.x*0.10;
  let flow=0.70+0.18*sin(w.x*0.42+w.z*0.27+drift)+0.12*sin(w.z*0.79-w.x*0.18-drift*1.3);
  let density=(0.003+0.030*exp(-max(w.y,0.)*1.65)*flow)*step;
  optical+=density;
  scattered+=vec3(0.038,0.083,0.090)*density;
 }
 let transmission=exp(-optical*amount);
 color=color*transmission+scattered*amount;
 // Analytic light scattering around the six braziers, clipped by scene depth.
 for(var i=0;i<6;i++) {
  let light=cam.lights[i].xyz;let along=dot(light-cam.eye.xyz,ray);
  let closest=cam.eye.xyz+ray*clamp(along,0.,distance);
  let perpendicular=length(light-closest);
  let visible=smoothstep(-0.5,0.5,along)*(1.-smoothstep(distance-0.4,distance+0.4,along));
  let glow=exp(-perpendicular*perpendicular/2.2)*visible/(1.+along*along*0.025);
  color+=vec3(0.22,0.082,0.018)*glow*amount;
 }
 // Broad and tight bloom lobes. Soft threshold retains flame shape and the
 // small bright details of enchanted weapons instead of whitening them out.
 var bloom=vec3(0.);
 let radii=array<f32,3>(2.5,8.,21.);
 let weights=array<f32,3>(0.52,0.32,0.16);
 for(var k=0;k<3;k++) {
  var lobe=vec3(0.);
  for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){
   let tap=textureSampleLevel(scene,scene_sampler,uv+vec2(f32(x),f32(y))*texel*radii[k],0.).rgb;
   let l=luminance(tap);let threshold=smoothstep(0.55,1.45,l);
   lobe+=tap*threshold;
  }}
  bloom+=lobe/9.*weights[k];
 }
 color+=bloom*0.43*amount;
 // Conservative edge smoothing on the 3D layer only; no blur on cards or text.
 let north=textureSampleLevel(scene,scene_sampler,uv+vec2(0.,texel.y),0.).rgb;
 let south=textureSampleLevel(scene,scene_sampler,uv-vec2(0.,texel.y),0.).rgb;
 let east=textureSampleLevel(scene,scene_sampler,uv+vec2(texel.x,0.),0.).rgb;
 let west=textureSampleLevel(scene,scene_sampler,uv-vec2(texel.x,0.),0.).rgb;
 let average=(north+south+east+west)*0.25;
 let contrast=abs(luminance(average)-luminance(original));
 color+=clamp(average-original,vec3(-0.12),vec3(0.12))*smoothstep(0.08,0.35,contrast)*0.22*amount;
 // Warm highlights, blue-green shadows, and a filmic shoulder preserve fire.
 let lumin=luminance(color);
 color*=mix(vec3(0.87,1.00,1.075),vec3(1.07,1.00,0.89),smoothstep(0.05,0.9,lumin));
 color=mix(vec3(lumin),color,1.08);
 let graded=filmic(max(color,vec3(0.))*1.18);
 let neutral=color/(color+vec3(0.84));
 var result=mix(neutral,graded,amount);
 result*=1.-0.30*pow(length((uv-0.5)*1.35),1.8);
 return vec4(result,1.);
}
