use crate::{
    game::{Game, Mode},
    scene::{self, Bones, Vertex},
};
use glam::{Mat4, Vec3};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

/// Placement of resident corpse geometry inside one vertex buffer. Pieces are
/// appended once, when they first appear. Expired pieces leave holes, and
/// when the buffer fills, every live piece is packed again from the start,
/// growing the buffer if needed.
#[derive(Default)]
struct PieceSlab {
    ranges: std::collections::HashMap<u64, std::ops::Range<u32>>,
    used: u32,
    capacity: u32,
}
/// What `PieceSlab::admit` needs the GPU to do.
#[derive(Debug, PartialEq)]
struct Admission {
    /// The buffer must be recreated with this many vertices (a reallocation
    /// discards every existing range, so `uploads` then covers all pieces).
    grow: Option<u32>,
    /// Pieces to upload, as (index into `live`, first vertex).
    uploads: Vec<(usize, u32)>,
}
impl PieceSlab {
    const MIN_CAPACITY: u32 = 1 << 16;
    /// Make every live piece, given as (id, vertex count), resident.
    fn admit(&mut self, live: &[(u64, u32)]) -> Admission {
        self.ranges
            .retain(|id, _| live.iter().any(|(live_id, _)| live_id == id));
        let missing: Vec<usize> = (0..live.len())
            .filter(|&i| !self.ranges.contains_key(&live[i].0))
            .collect();
        let needed: u32 = missing.iter().map(|&i| live[i].1).sum();
        if self.used + needed <= self.capacity {
            let uploads = missing
                .into_iter()
                .map(|i| {
                    let start = self.used;
                    self.used += live[i].1;
                    self.ranges.insert(live[i].0, start..self.used);
                    (i, start)
                })
                .collect();
            return Admission { grow: None, uploads };
        }
        // Full: pack every live piece again, with room to spare for new ones.
        let total: u32 = live.iter().map(|piece| piece.1).sum();
        let grow = (total > self.capacity / 2).then(|| {
            self.capacity = (total * 2).next_power_of_two().max(Self::MIN_CAPACITY);
            self.capacity
        });
        self.ranges.clear();
        self.used = 0;
        let uploads = (0..live.len())
            .map(|i| {
                let start = self.used;
                self.used += live[i].1;
                self.ranges.insert(live[i].0, start..self.used);
                (i, start)
            })
            .collect();
        Admission { grow, uploads }
    }
}

/// Corpse sections on the GPU: geometry placed by the slab, plus this
/// frame's transforms.
struct ResidentPieces {
    slab: PieceSlab,
    geometry: wgpu::Buffer,
    instances: wgpu::Buffer,
}
impl ResidentPieces {
    /// Upload new corpse sections once, and this frame's section transforms.
    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bones: &Bones,
        transforms: &[scene::PieceInstance],
    ) {
        let live: Vec<(u64, u32)> = bones
            .pieces
            .iter()
            .map(|piece| (piece.id, piece.vertices.len() as u32))
            .collect();
        let admission = self.slab.admit(&live);
        let stride = std::mem::size_of::<Vertex>() as u64;
        if let Some(capacity) = admission.grow {
            self.geometry = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Resident corpse sections"),
                size: capacity as u64 * stride,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        for (index, start) in admission.uploads {
            queue.write_buffer(
                &self.geometry,
                start as u64 * stride,
                bytemuck::cast_slice(&bones.pieces[index].vertices),
            );
        }
        let bytes: &[u8] = bytemuck::cast_slice(transforms);
        if bytes.len() as u64 > self.instances.size() {
            self.instances = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Corpse section transforms"),
                size: (bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.instances, 0, bytes);
    }
}

fn index_enemy_geometry(vertices: &[Vertex]) -> (Vec<Vertex>, Vec<u32>) {
    let mut unique = Vec::new();
    let mut indices = Vec::with_capacity(vertices.len());
    let mut lookup = std::collections::HashMap::<[u32; 15], u32>::new();
    for vertex in vertices {
        let key: [u32; 15] = bytemuck::cast(*vertex);
        let index = *lookup.entry(key).or_insert_with(|| {
            let index = unique.len() as u32;
            unique.push(*vertex);
            index
        });
        indices.push(index);
    }
    (unique, indices)
}

struct WorldChunk {
    indices: std::ops::Range<u32>,
    center: Vec3,
    radius: f32,
}
/// Spatially batch static triangles once. Bounds include every vertex (even
/// terrain triangles spanning cells), plus a margin for wind animated foliage.
fn index_world_geometry(vertices: &[Vertex]) -> (Vec<Vertex>, Vec<u32>, Vec<WorldChunk>) {
    let mut cells = std::collections::BTreeMap::<(i32, i32), Vec<Vertex>>::new();
    for tri in vertices.chunks_exact(3) {
        let center = tri.iter().map(|v| Vec3::from_array(v.pos)).sum::<Vec3>() / 3.;
        let key = (
            (center.x / 16.).floor() as i32,
            (center.z / 16.).floor() as i32,
        );
        cells.entry(key).or_default().extend_from_slice(tri);
    }
    let mut reordered = Vec::with_capacity(vertices.len());
    let mut chunks = Vec::with_capacity(cells.len());
    for cell in cells.values() {
        let low = cell
            .iter()
            .fold(Vec3::splat(f32::MAX), |a, v| a.min(Vec3::from_array(v.pos)));
        let high = cell
            .iter()
            .fold(Vec3::splat(f32::MIN), |a, v| a.max(Vec3::from_array(v.pos)));
        let start = reordered.len() as u32;
        reordered.extend_from_slice(cell);
        chunks.push(WorldChunk {
            indices: start..reordered.len() as u32,
            center: (low + high) * 0.5,
            radius: (high - low).length() * 0.5 + 1.,
        });
    }
    let (unique, indices) = index_enemy_geometry(&reordered);
    (unique, indices, chunks)
}

#[cfg(test)]
mod model_tests {
    use super::*;

    /// Every live piece has its own range of the right length inside the buffer.
    fn check(slab: &PieceSlab, live: &[(u64, u32)]) {
        assert_eq!(slab.ranges.len(), live.len());
        let mut ranges: Vec<_> = live
            .iter()
            .map(|(id, len)| {
                let range = slab.ranges[id].clone();
                assert_eq!(range.len() as u32, *len);
                range
            })
            .collect();
        ranges.sort_by_key(|r| r.start);
        assert!(ranges.windows(2).all(|w| w[0].end <= w[1].start));
        assert!(ranges.last().is_none_or(|r| r.end <= slab.used && slab.used <= slab.capacity));
    }
    #[test]
    fn corpse_slab_appends_reuses_space_and_compacts_without_losing_a_piece() {
        let mut slab = PieceSlab::default();
        // The first corpse sizes the buffer and uploads everything.
        let mut live = vec![(1, 40_000), (2, 9_000)];
        let first = slab.admit(&live);
        assert_eq!(first.grow, Some(PieceSlab::MIN_CAPACITY << 1));
        assert_eq!(first.uploads, vec![(0, 0), (1, 40_000)]);
        check(&slab, &live);
        // Nothing new: nothing to upload.
        assert_eq!(slab.admit(&live).uploads, vec![]);
        // A new piece is appended after the others; an expired one leaves a hole.
        live = vec![(2, 9_000), (3, 30_000)];
        let next = slab.admit(&live);
        assert_eq!(next, Admission { grow: None, uploads: vec![(1, 49_000)] });
        check(&slab, &live);
        assert_eq!(slab.ranges[&2], 40_000..49_000);
        // Another piece expires and the next doesn't fit after the others, so
        // the live pieces are packed again from the start of the same buffer.
        live = vec![(2, 9_000), (4, 53_000)];
        let packed = slab.admit(&live);
        assert_eq!(packed.grow, None);
        assert_eq!(packed.uploads, vec![(0, 0), (1, 9_000)]);
        check(&slab, &live);
        // More live geometry than half the buffer grows it.
        live.push((5, 70_000));
        let grown = slab.admit(&live);
        assert_eq!(grown.grow, Some(1 << 19));
        assert_eq!(grown.uploads.len(), live.len());
        check(&slab, &live);
        // Churn: pieces come and go, and every live piece stays placed.
        let mut next_id = 6;
        for step in 0..400u32 {
            if live.len() > 6 || step % 3 == 0 {
                live.remove((step as usize * 7) % live.len());
            }
            live.push((next_id, 2_000 + (step * 7919) % 40_000));
            next_id += 1;
            let admission = slab.admit(&live);
            for (index, start) in admission.uploads {
                assert_eq!(slab.ranges[&live[index].0].start, start);
            }
            check(&slab, &live);
        }
    }

    #[test]
    fn world_chunks_preserve_triangle_attributes_and_contain_their_geometry() {
        let mut mesh = scene::Mesh::new();
        for p in [
            Vec3::new(-19., 0., -4.),
            Vec3::new(35., 3., 25.),
            Vec3::ZERO,
        ] {
            mesh.cube(p, Vec3::new(30., 5., 2.), [0.2, 0.3, 0.4], 1.);
        }
        let (unique, indices, chunks) = index_world_geometry(&mesh.vertices);
        assert_eq!(indices.len(), mesh.vertices.len());
        let mut actual = Vec::new();
        for chunk in chunks {
            assert_eq!(chunk.indices.len() % 3, 0);
            for i in chunk.indices {
                let v = unique[indices[i as usize] as usize];
                assert!(Vec3::from_array(v.pos).distance(chunk.center) < chunk.radius);
                actual.push(bytemuck::cast::<Vertex, [u32; 15]>(v));
            }
        }
        let mut expected: Vec<[u32; 15]> = mesh.vertices.into_iter().map(bytemuck::cast).collect();
        expected.sort_unstable();
        actual.sort_unstable();
        assert_eq!(actual, expected);
    }

    #[test]
    fn indexed_enemy_geometry_preserves_every_attribute_and_species_range() {
        let (expanded, ranges) = crate::enemy_assets::gpu_geometry();
        let (unique, indices) = index_enemy_geometry(&expanded);
        assert!(unique.len() < expanded.len());
        assert_eq!(indices.len(), expanded.len());
        for range in ranges {
            assert_eq!(range.len() % 3, 0);
            for index in range {
                let actual: [u32; 15] = bytemuck::cast(unique[indices[index as usize] as usize]);
                let expected: [u32; 15] = bytemuck::cast(expanded[index as usize]);
                assert_eq!(actual, expected);
            }
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Camera {
    vp: [[f32; 4]; 4],
    inverse_vp: [[f32; 4]; 4],
    eye: [f32; 4],
    info: [f32; 4],
    lights: [[f32; 4]; 6],
}
pub struct Renderer {
    pub timings: [f64; 3],
    pub vertex_count: usize,
    pub optimized: bool,
    model_review_cpu: bool,
    offscreen_target: Option<wgpu::Texture>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    world: wgpu::RenderPipeline,
    transparent: wgpu::RenderPipeline,
    sphere_pipeline: wgpu::RenderPipeline,
    sphere_geometry: wgpu::Buffer,
    sphere_instances: wgpu::Buffer,
    sphere_count: u32,
    enemy_pipeline: wgpu::RenderPipeline,
    enemy_geometry: wgpu::Buffer,
    enemy_indices: wgpu::Buffer,
    enemy_ranges: Vec<std::ops::Range<u32>>,
    enemy_instances: wgpu::Buffer,
    enemy_bind: wgpu::BindGroup,
    enemy_layout: wgpu::BindGroupLayout,
    piece_pipeline: wgpu::RenderPipeline,
    pieces: ResidentPieces,
    dynamic_mesh: scene::Mesh,
    sky: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind: wgpu::BindGroup,
    camera_layout: wgpu::BindGroupLayout,
    material_view: wgpu::TextureView,
    material_sampler: wgpu::Sampler,
    weapon_color: wgpu::TextureView,
    weapon_surface: wgpu::TextureView,
    static_buffer: wgpu::Buffer,
    static_indices: wgpu::Buffer,
    static_chunks: Vec<WorldChunk>,
    dynamic_buffer: wgpu::Buffer,
    scene_view: wgpu::TextureView,
    depth: wgpu::TextureView,
    scene_bind: wgpu::BindGroup,
    scene_layout: wgpu::BindGroupLayout,
    pub egui: egui_wgpu::Renderer,
    pub view_projection: Mat4,
}
impl Renderer {
    pub async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL | wgpu::Backends::VULKAN | wgpu::Backends::DX12,
            ..Default::default()
        });
        let surface = instance
            .create_surface(window.clone())
            .expect("create GPU surface");
        crate::watchdog::milestone("GPU surface created");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("No compatible GPU");
        crate::watchdog::milestone("GPU adapter selected");
        println!("GPU: {:?}", adapter.get_info());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Gravewake renderer"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .expect("GPU device");
        crate::watchdog::milestone("GPU device ready");
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        println!(
            "Presentation modes: {:?}; display refresh: {:?}",
            caps.present_modes,
            window
                .current_monitor()
                .and_then(|m| m.refresh_rate_millihertz())
        );
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        crate::watchdog::milestone("surface configured");
        let offscreen_target = (std::env::args()
            .any(|arg| arg == "--model-offscreen" || arg == "--world-offscreen")
            && std::env::args().any(|arg| arg == "--model-review" || arg == "--world-review"))
        .then(|| Self::review_target(&device, &config));
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera and lights"),
            size: std::mem::size_of::<Camera>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pixels = image::load_from_memory(include_bytes!("../assets/material-atlas.png"))
            .expect("material atlas")
            .to_rgba8();
        let levels = 32 - pixels.width().max(pixels.height()).leading_zeros();
        let mut mip_data = pixels.as_raw().clone();
        let mut mip = pixels.clone();
        for _ in 1..levels {
            mip = image::imageops::resize(
                &mip,
                (mip.width() / 2).max(1),
                (mip.height() / 2).max(1),
                image::imageops::FilterType::Triangle,
            );
            mip_data.extend_from_slice(mip.as_raw());
        }
        let texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("Gothic material atlas"),
                size: wgpu::Extent3d {
                    width: pixels.width(),
                    height: pixels.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &mip_data,
        );
        let material_view = texture.create_view(&Default::default());
        let material_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_max_clamp: 6.,
            ..Default::default()
        });
        let weapon_texture = |label, bytes: &[u8], format| {
            let pixels = image::load_from_memory(bytes)
                .expect("embedded Blender material")
                .to_rgba8();
            let levels = 32 - pixels.width().max(pixels.height()).leading_zeros();
            let mut mip = pixels.clone();
            let mut data = mip.as_raw().clone();
            for _ in 1..levels {
                mip = image::imageops::resize(
                    &mip,
                    (mip.width() / 2).max(1),
                    (mip.height() / 2).max(1),
                    image::imageops::FilterType::Triangle,
                );
                data.extend_from_slice(mip.as_raw());
            }
            device
                .create_texture_with_data(
                    &queue,
                    &wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d {
                            width: pixels.width(),
                            height: pixels.height(),
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: levels,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    },
                    wgpu::util::TextureDataOrder::LayerMajor,
                    &data,
                )
                .create_view(&Default::default())
        };
        let weapon_color = weapon_texture(
            "Blender baked arsenal albedo",
            include_bytes!("../assets/weapons/weathered-color.png"),
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );
        let weapon_surface = weapon_texture(
            "Blender baked roughness height metalness",
            include_bytes!("../assets/weapons/weathered-surface.png"),
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let camera_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &camera_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&material_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&material_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&weapon_color),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&weapon_surface),
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&camera_layout],
            push_constant_ranges: &[],
        });
        let enemy_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Articulated enemy rigs"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let enemy_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Instanced Blender enemy pipeline"),
                bind_group_layouts: &[&camera_layout, &enemy_layout],
                push_constant_ranges: &[],
            });
        let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/world.wgsl"));
        let targets = [Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba16Float,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let attributes = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x3,3=>Float32,4=>Float32x3,5=>Float32x2];
        let buffers = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        }];
        let make_pipeline = |vs,
                             fs,
                             buffers: &[wgpu::VertexBufferLayout<'_>],
                             depth_write,
                             pipeline_layout: &wgpu::PipelineLayout| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(vs),
                layout: Some(pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &targets,
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: depth_write,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let world = make_pipeline("vs", "fs", &buffers, true, &layout);
        // Halos and ground shadows blend after solids and never write depth.
        // Fog/occlusion must reconstruct the surface behind a soft billboard.
        let transparent = make_pipeline("vs", "fs", &buffers, false, &layout);
        let enemy_pipeline =
            make_pipeline("enemy_vs", "fs", &buffers, true, &enemy_pipeline_layout);
        let (enemy_vertices, enemy_ranges) = crate::enemy_assets::gpu_geometry();
        // The authoring format is an expanded triangle list. Deduplicate the
        // complete vertex (including rig group, normals and UV seams) once so
        // the GPU vertex cache can reuse expensive articulated transforms.
        let (enemy_unique, enemy_indices) = index_enemy_geometry(&enemy_vertices);
        println!(
            "Resident enemy geometry: {} triangles, {} unique vertices ({} expanded)",
            enemy_indices.len() / 3,
            enemy_unique.len(),
            enemy_vertices.len()
        );
        let enemy_geometry = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Resident Blender enemy topology"),
            contents: bytemuck::cast_slice(&enemy_unique),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let enemy_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Resident Blender enemy triangles"),
            contents: bytemuck::cast_slice(&enemy_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let enemy_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Animated enemy rigs"),
            size: (std::mem::size_of::<crate::enemy_assets::Instance>() * 128) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let enemy_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Animated enemy rigs"),
            layout: &enemy_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: enemy_instances.as_entire_binding(),
            }],
        });
        let instance_attributes = wgpu::vertex_attr_array![6=>Float32x4,7=>Float32x4,8=>Float32x4,9=>Float32x4,10=>Float32x4,11=>Float32x4,12=>Float32x4];
        let sphere_pipeline = make_pipeline(
            "sphere_vs",
            "fs",
            &[
                buffers[0].clone(),
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<scene::SphereInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &instance_attributes,
                },
            ],
            true,
            &layout,
        );
        let mut sphere = scene::Mesh::new();
        sphere.ellipsoid(Vec3::ZERO, Vec3::ONE, [1.; 3], 2.);
        let sphere_count = sphere.vertices.len() as u32;
        let sphere_geometry = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shared enemy sphere"),
            contents: bytemuck::cast_slice(&sphere.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let sphere_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Enemy shape transforms"),
            size: 2 * 1024 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let piece_attributes =
            wgpu::vertex_attr_array![6=>Float32x4,7=>Float32x4,8=>Float32x4,9=>Float32x4];
        let piece_pipeline = make_pipeline(
            "piece_vs",
            "fs",
            &[
                buffers[0].clone(),
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<scene::PieceInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &piece_attributes,
                },
            ],
            true,
            &layout,
        );
        // Placeholders until the first corpse appears; both grow on demand.
        let piece_geometry = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Resident corpse sections"),
            size: std::mem::size_of::<Vertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let piece_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Corpse section transforms"),
            size: (std::mem::size_of::<scene::PieceInstance>() * 256) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sky = make_pipeline("sky_vs", "sky_fs", &[], false, &layout);
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let (scene_view, depth, scene_bind) =
            Self::targets(&device, &scene_layout, &config, &camera_buffer);
        let post_shader =
            device.create_shader_module(wgpu::include_wgsl!("../shaders/composite.wgsl"));
        let post_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&scene_layout],
            push_constant_ranges: &[],
        });
        let composite = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Hollowlight depth-aware cinematic composite"),
            layout: Some(&post_layout),
            vertex: wgpu::VertexState {
                module: &post_shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &post_shader,
                entry_point: Some("fs"),
                targets: &[Some(config.format.into())],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let mesh = scene::world();
        let (static_vertices, indices, static_chunks) = index_world_geometry(&mesh.vertices);
        println!(
            "World geometry: {} triangles, {} unique vertices in {} visibility chunks",
            indices.len() / 3,
            static_vertices.len(),
            static_chunks.len()
        );
        let static_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Indexed expanded Mournhollow"),
            contents: bytemuck::cast_slice(&static_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let static_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Expanded world triangle indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let dynamic_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Animated geometry"),
            size: 16 * 1024 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let egui = egui_wgpu::Renderer::new(&device, format, None, 1, false);
        Self {
            surface,
            device,
            queue,
            config,
            world,
            transparent,
            sphere_pipeline,
            sphere_geometry,
            sphere_instances,
            sphere_count,
            enemy_pipeline,
            enemy_geometry,
            enemy_indices,
            enemy_ranges,
            enemy_instances,
            enemy_bind,
            enemy_layout,
            piece_pipeline,
            pieces: ResidentPieces {
                slab: PieceSlab::default(),
                geometry: piece_geometry,
                instances: piece_instances,
            },
            dynamic_mesh: scene::Mesh {
                cpu_pieces: std::env::var_os("GRAVEWAKE_CPU_CORPSES").is_some(),
                ..scene::Mesh::new()
            },
            sky,
            composite,
            camera_buffer,
            camera_bind,
            camera_layout,
            material_view,
            material_sampler,
            weapon_color,
            weapon_surface,
            static_buffer,
            static_indices,
            static_chunks,
            timings: [0.; 3],
            vertex_count: 0,
            optimized: true,
            model_review_cpu: std::env::args().any(|arg| arg == "--model-cpu")
                && std::env::args().any(|arg| arg == "--model-review"),
            offscreen_target,
            dynamic_buffer,
            scene_view,
            depth,
            scene_bind,
            scene_layout,
            egui,
            view_projection: Mat4::IDENTITY,
        }
    }
    pub fn register_previews(&mut self, ctx: &egui::Context) {
        for kind in 0..=crate::weapons::WeaponKind::ALL.len() {
            for rarity in 0..4 {
                let mut mesh = scene::Mesh::new();
                if kind == crate::weapons::WeaponKind::ALL.len() {
                    scene::chalice(&mut mesh);
                } else {
                    scene::weapon(&mut mesh, crate::weapons::WeaponKind::ALL[kind], rarity);
                }
                let vb = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Weapon thumbnail"),
                        contents: bytemuck::cast_slice(&mesh.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                let min = mesh
                    .vertices
                    .iter()
                    .fold(Vec3::splat(f32::MAX), |a, v| a.min(Vec3::from_array(v.pos)));
                let max = mesh
                    .vertices
                    .iter()
                    .fold(Vec3::splat(f32::MIN), |a, v| a.max(Vec3::from_array(v.pos)));
                let target = (min + max) * 0.5;
                let eye = target + Vec3::new(-1.8, 0.85, -1.25);
                let item_view = Mat4::look_at_rh(eye, target, Vec3::new(0.08, 1., 0.));
                let radius = mesh
                    .vertices
                    .iter()
                    .map(|v| {
                        let q = item_view.transform_point3(Vec3::from_array(v.pos));
                        q.x.abs().max(q.y.abs() / 0.75)
                    })
                    .fold(0.0_f32, f32::max)
                    * 1.12;
                let vp = Mat4::orthographic_rh(
                    -radius,
                    radius,
                    -radius * 0.75,
                    radius * 0.75,
                    0.01,
                    10.,
                ) * item_view;
                let camera = Camera {
                    vp: vp.to_cols_array_2d(),
                    inverse_vp: vp.inverse().to_cols_array_2d(),
                    eye: eye.extend(1.).to_array(),
                    info: [0., 1., 1., 0.],
                    lights: [[0.; 4]; 6],
                };
                let cb = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: bytemuck::bytes_of(&camera),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &self.camera_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: cb.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&self.material_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.material_sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(&self.weapon_color),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(&self.weapon_surface),
                        },
                    ],
                });
                let size = wgpu::Extent3d {
                    width: 512,
                    height: 384,
                    depth_or_array_layers: 1,
                };
                let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Card weapon render"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let view = tex.create_view(&Default::default());
                let depth = self
                    .device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: None,
                        size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Depth32Float,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                    .create_view(&Default::default());
                let mut encoder = self.device.create_command_encoder(&Default::default());
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Render real weapon for card"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &depth,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    pass.set_pipeline(&self.world);
                    pass.set_bind_group(0, &bind, &[]);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.draw(0..mesh.vertices.len() as u32, 0..1);
                }
                self.queue.submit([encoder.finish()]);
                let id = self.egui.register_native_texture(
                    &self.device,
                    &view,
                    wgpu::FilterMode::Linear,
                );
                ctx.data_mut(|d| {
                    d.insert_temp(egui::Id::new(("weapon_preview", kind, rarity)), id)
                });
            }
        }
    }
    fn targets(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        config: &wgpu::SurfaceConfiguration,
        camera: &wgpu::Buffer,
    ) -> (wgpu::TextureView, wgpu::TextureView, wgpu::BindGroup) {
        let width = config.width.clamp(640, 1440);
        let height = (width as f32 * config.height as f32 / config.width as f32) as u32;
        let size = wgpu::Extent3d {
            width,
            height: height.max(1),
            depth_or_array_layers: 1,
        };
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Low resolution scene"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Depth"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&depth),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: camera.as_entire_binding(),
                },
            ],
        });
        (view, depth, bind)
    }
    fn review_target(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Review-only offscreen presentation target"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        if self.offscreen_target.is_some() {
            self.offscreen_target = Some(Self::review_target(&self.device, &self.config));
        }
        (self.scene_view, self.depth, self.scene_bind) = Self::targets(
            &self.device,
            &self.scene_layout,
            &self.config,
            &self.camera_buffer,
        );
    }
    pub fn camera(&mut self, game: &Game) {
        let mode = if game.vsync {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        if mode != self.config.present_mode {
            self.config.present_mode = mode;
            self.surface.configure(&self.device, &self.config);
        }

        let first_person = matches!(
            game.mode,
            Mode::Arena | Mode::LevelUp | Mode::Paused | Mode::Dead | Mode::Victory
        );
        let (eye, target) = if first_person {
            (
                game.run.pos,
                game.run.pos
                    + game.forward()
                    + Vec3::Y
                        * crate::motion::recoil(game.shot_age)
                        * if game.run.weapon.kind == crate::game::WeaponKind::Double {
                            0.012
                        } else {
                            0.006
                        },
            )
        } else if game.mode == Mode::Bestiary {
            if game.bestiary_index == 5 {
                (Vec3::new(1.6, 1.15, 2.7), Vec3::new(0., 0.4, 0.))
            } else {
                (Vec3::new(3.5, 2., 5.), Vec3::new(0., 1.4, 0.))
            }
        } else {
            (
                Vec3::new(4.5 + (game.elapsed * 0.04).sin() * 0.4, 2.5, 7.),
                Vec3::new(-4.5, 1.4, -3.),
            )
        };
        let view = Mat4::look_at_rh(eye, target, Vec3::Y);
        let projection = Mat4::perspective_rh(
            if first_person { game.prefs.fov } else { 70. }.to_radians(),
            self.config.width as f32 / self.config.height as f32,
            0.04,
            200.,
        );
        self.view_projection = projection * view;
        let camera = Camera {
            vp: self.view_projection.to_cols_array_2d(),
            inverse_vp: self.view_projection.inverse().to_cols_array_2d(),
            eye: eye.extend(1.).to_array(),
            info: [
                game.elapsed,
                0.,
                game.shader_intensity,
                (game.flash / 0.095).clamp(0., 1.)
                    * if game.prefs.reduce_flashes { 0.35 } else { 1. },
            ],
            lights: scene::nearest_lights(eye),
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera));
    }
    pub fn render(
        &mut self,
        game: &Game,
        bones: &Bones,
        ctx: &egui::Context,
        output: egui::FullOutput,
        capture: Option<&str>,
    ) -> Result<(), wgpu::SurfaceError> {
        let acquire = std::time::Instant::now();
        crate::watchdog::milestone("acquiring surface texture");
        let frame = if self.offscreen_target.is_none() {
            Some(self.surface.get_current_texture()?)
        } else {
            None
        };
        crate::watchdog::milestone("drawing frame");
        self.timings[1] = acquire.elapsed().as_secs_f64() * 1000.;
        let output_texture = frame
            .as_ref()
            .map(|frame| &frame.texture)
            .or(self.offscreen_target.as_ref())
            .unwrap();
        let view = output_texture.create_view(&Default::default());
        let mesh_start = std::time::Instant::now();
        scene::dynamic(
            game,
            bones,
            &mut self.dynamic_mesh,
            self.view_projection,
            self.optimized && !self.model_review_cpu,
        );
        if !self.dynamic_mesh.pieces.is_empty() {
            self.pieces.upload(
                &self.device,
                &self.queue,
                bones,
                &self.dynamic_mesh.piece_instances,
            );
        }
        let mesh = &self.dynamic_mesh;
        self.timings[0] = mesh_start.elapsed().as_secs_f64() * 1000.;
        self.vertex_count = mesh.vertices.len() + mesh.transparent.len();
        let submit = std::time::Instant::now();
        let required = (self.vertex_count * std::mem::size_of::<scene::Vertex>()) as u64;
        if required > self.dynamic_buffer.size() {
            self.dynamic_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Growing arena geometry"),
                size: required.next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.queue.write_buffer(
            &self.dynamic_buffer,
            0,
            bytemuck::cast_slice(&mesh.vertices),
        );
        self.queue.write_buffer(
            &self.dynamic_buffer,
            (mesh.vertices.len() * std::mem::size_of::<scene::Vertex>()) as u64,
            bytemuck::cast_slice(&mesh.transparent),
        );
        let sphere_bytes = bytemuck::cast_slice(&mesh.spheres);
        if sphere_bytes.len() as u64 > self.sphere_instances.size() {
            self.sphere_instances = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Growing enemy transforms"),
                size: (sphere_bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !sphere_bytes.is_empty() {
            self.queue
                .write_buffer(&self.sphere_instances, 0, sphere_bytes);
        }
        let enemy_bytes = bytemuck::cast_slice(&mesh.enemies);
        if enemy_bytes.len() as u64 > self.enemy_instances.size() {
            self.enemy_instances = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Growing animated enemy rigs"),
                size: (enemy_bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.enemy_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Growing animated enemy rigs"),
                layout: &self.enemy_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.enemy_instances.as_entire_binding(),
                }],
            });
        }
        if !enemy_bytes.is_empty() {
            self.queue
                .write_buffer(&self.enemy_instances, 0, enemy_bytes);
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Gravewake frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("World"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.camera_bind, &[]);
            pass.set_pipeline(&self.sky);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&self.world);
            pass.set_vertex_buffer(0, self.static_buffer.slice(..));
            pass.set_index_buffer(self.static_indices.slice(..), wgpu::IndexFormat::Uint32);
            let visible = scene::Visibility::new(self.view_projection);
            for chunk in &self.static_chunks {
                if !self.optimized || visible.contains(chunk.center, chunk.radius) {
                    pass.draw_indexed(chunk.indices.clone(), 0, 0..1);
                }
            }
            if !mesh.enemies.is_empty() {
                pass.set_pipeline(&self.enemy_pipeline);
                pass.set_bind_group(1, &self.enemy_bind, &[]);
                pass.set_vertex_buffer(0, self.enemy_geometry.slice(..));
                pass.set_index_buffer(self.enemy_indices.slice(..), wgpu::IndexFormat::Uint32);
                for (index, enemy) in mesh.enemies.iter().enumerate() {
                    let range = self.enemy_ranges[enemy.params[0] as usize].clone();
                    pass.draw_indexed(range, 0, index as u32..index as u32 + 1);
                }
                pass.set_pipeline(&self.world);
            }
            if !mesh.pieces.is_empty() {
                pass.set_pipeline(&self.piece_pipeline);
                pass.set_vertex_buffer(0, self.pieces.geometry.slice(..));
                pass.set_vertex_buffer(1, self.pieces.instances.slice(..));
                for (index, id) in mesh.pieces.iter().enumerate() {
                    let range = self.pieces.slab.ranges[id].clone();
                    pass.draw(range, index as u32..index as u32 + 1);
                }
                pass.set_pipeline(&self.world);
            }
            if !mesh.spheres.is_empty() {
                pass.set_pipeline(&self.sphere_pipeline);
                pass.set_vertex_buffer(0, self.sphere_geometry.slice(..));
                pass.set_vertex_buffer(1, self.sphere_instances.slice(..));
                pass.draw(0..self.sphere_count, 0..mesh.spheres.len() as u32);
                pass.set_pipeline(&self.world);
            }
            pass.set_vertex_buffer(0, self.dynamic_buffer.slice(..));
            pass.draw(0..mesh.vertices.len() as u32, 0..1);
            pass.set_pipeline(&self.transparent);
            pass.draw(mesh.vertices.len() as u32..self.vertex_count as u32, 0..1);
        }
        let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: output.pixels_per_point,
        };
        for (id, delta) in &output.textures_delta.set {
            self.egui
                .update_texture(&self.device, &self.queue, *id, delta);
        }
        let command_buffers =
            self.egui
                .update_buffers(&self.device, &self.queue, &mut encoder, &jobs, &screen);
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Composite and interface"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            pass.set_pipeline(&self.composite);
            pass.set_bind_group(0, &self.scene_bind, &[]);
            pass.draw(0..3, 0..1);
            self.egui.render(&mut pass, &jobs, &screen);
        }
        let padded = (self.config.width * 4).div_ceil(256) * 256;
        let readback = capture.map(|_| {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Screenshot readback"),
                size: padded as u64 * self.config.height as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: output_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded),
                        rows_per_image: None,
                    },
                },
                wgpu::Extent3d {
                    width: self.config.width,
                    height: self.config.height,
                    depth_or_array_layers: 1,
                },
            );
            buffer
        });
        self.queue.submit(
            command_buffers
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        if let (Some(path), Some(buffer)) = (capture, readback) {
            let (tx, rx) = std::sync::mpsc::channel();
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = tx.send(result);
                });
            crate::watchdog::milestone("reading back screenshot");
            let _ = self.device.poll(wgpu::PollType::Wait);
            if rx.recv().unwrap().is_ok() {
                let data = buffer.slice(..).get_mapped_range();
                let mut pixels = vec![];
                for row in data.chunks(padded as usize) {
                    pixels.extend_from_slice(&row[..self.config.width as usize * 4]);
                }
                if matches!(
                    self.config.format,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
                ) {
                    for p in pixels.chunks_exact_mut(4) {
                        p.swap(0, 2);
                    }
                }
                if let Some(parent) = std::path::Path::new(path).parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                image::save_buffer(
                    path,
                    &pixels,
                    self.config.width,
                    self.config.height,
                    image::ColorType::Rgba8,
                )
                .expect("save screenshot");
                println!("Captured {path}");
            }
        }
        // Diagnostic only: finish each submitted frame to measure serial
        // CPU+GPU work without drawable/compositor pacing or an unbounded queue.
        if self.offscreen_target.is_some() {
            let _ = self.device.poll(wgpu::PollType::Wait);
        }
        if let Some(frame) = frame {
            crate::watchdog::milestone("presenting");
            frame.present();
        }
        self.timings[2] = submit.elapsed().as_secs_f64() * 1000.;
        for id in output.textures_delta.free {
            self.egui.free_texture(&id);
        }
        Ok(())
    }
}
