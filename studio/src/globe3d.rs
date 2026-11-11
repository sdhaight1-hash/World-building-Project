//! Native wgpu-backed, height-displaced 3D globe mesh.
//!
//! A globe mesh is built from the same DEM and synthetic geographic reference
//! that the 2D cartographic atlas reads. The GPU transforms its latitude,
//! longitude and metre elevations in WGSL; map colors use the same native
//! layer styling texture as the atlas. This is NOT a voxel mesh.
//!
//! It is an initial static grid tessellation. The next scale milestone can
//! add quadtree LOD, terrain normals and depth/camera improvements.
use crate::world::World;
use bytemuck::{Pod, Zeroable};
use eframe::egui;
use egui_wgpu::{CallbackResources, CallbackTrait, ScreenDescriptor};
use std::sync::Arc;
use wgpu::util::DeviceExt as _;

const DEFAULT_RADIUS_M: f32 = 6_371_000.0;
const GLOBE_SCALE: f32 = 0.94;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GlobeVertex {
    pub lon_lat: [f32; 2],
    pub elevation_m: f32,
    pub uv: [f32; 2],
}
const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32, 2 => Float32x2];

#[derive(Debug)]
pub struct GlobeMesh {
    pub vertices: Vec<GlobeVertex>,
    pub indices: Vec<u32>,
    pub lon_segments: usize,
    pub lat_segments: usize,
}

impl GlobeMesh {
    pub fn from_world(world: &World) -> Self {
        let lon_segments = (world.width / 2).clamp(64, 720);
        let lat_segments = (world.height / 2).clamp(32, 360);
        let mut vertices = Vec::with_capacity((lon_segments + 1) * (lat_segments + 1));
        let mut indices = Vec::with_capacity(lon_segments * lat_segments * 6);

        // UV follows Aegaea's source convention, NOT Earth's north-up.
        // x increases westward (+180 -> -180), y increases northward.
        for y in 0..=lat_segments {
            let v = y as f64 / lat_segments as f64;
            let lat = -90.0 + 180.0 * v;
            for x in 0..=lon_segments {
                let u = x as f64 / lon_segments as f64;
                let lon = 180.0 - 360.0 * u;
                let elevation_m = world
                    .elevation
                    .as_ref()
                    .map(|grid| grid.geo(lon, lat))
                    .filter(|v| v.is_finite())
                    .unwrap_or(0.0);
                vertices.push(GlobeVertex {
                    lon_lat: [(lon.to_radians()) as f32, (lat.to_radians()) as f32],
                    elevation_m,
                    uv: [u as f32, v as f32],
                });
            }
        }

        let stride = lon_segments + 1;
        for y in 0..lat_segments {
            for x in 0..lon_segments {
                let a = (y * stride + x) as u32;
                let b = a + 1;
                let c = a + stride as u32;
                let d = c + 1;
                indices.extend_from_slice(&[a, b, c, b, d, c]);
            }
        }
        Self {
            vertices,
            indices,
            lon_segments,
            lat_segments,
        }
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    angles: [f32; 4],
    shape: [f32; 4],
}

struct GlobeGpu {
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    uniform: wgpu::Buffer,
    texture: wgpu::Texture,
    bindings: wgpu::BindGroup,
    image_version: u64,
}

impl GlobeGpu {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &GlobeMesh,
        width: u32,
        height: u32,
        pixels: &[u8],
        version: u64,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aegaea-globe-vertices"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aegaea-globe-indices"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let initial = CameraUniform {
            angles: [0.0; 4],
            shape: [10.0, DEFAULT_RADIUS_M, GLOBE_SCALE, 0.0],
        };
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aegaea-globe-camera"),
            contents: bytemuck::bytes_of(&initial),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aegaea-globe-native-styled-atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        upload_map(queue, &texture, width, height, pixels);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("aegaea-globe-atlas-filter"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("aegaea-globe-bind-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
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
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aegaea-globe-bind-group"),
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aegaea-height-displaced-globe"),
            source: wgpu::ShaderSource::Wgsl(include_str!("globe3d.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("aegaea-globe-pipeline-layout"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("aegaea-globe-3d"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GlobeVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &ATTRIBUTES,
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // fragment discards the back hemisphere.
                ..Default::default()
            },
            depth_stencil: None, // egui's ordinary paint pass has no depth buffer.
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self {
            pipeline,
            vertices,
            indices,
            index_count: mesh.indices.len() as u32,
            uniform,
            texture,
            bindings,
            image_version: version,
        }
    }
}

fn upload_map(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    pixels: &[u8],
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

/// Created for each egui viewport repaint. All long-lived GPU buffers and
/// pipeline objects are cached inside egui_wgpu::CallbackResources.
pub struct GlobeCallback {
    pub mesh: Arc<GlobeMesh>,
    pub map_rgba: Arc<Vec<u8>>,
    pub map_size: [u32; 2],
    pub map_version: u64,
    pub target_format: wgpu::TextureFormat,
    pub center_lon_deg: f64,
    pub center_lat_deg: f64,
    pub vertical_exaggeration: f32,
}

impl CallbackTrait for GlobeCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if resources.get::<GlobeGpu>().is_none() {
            resources.insert(GlobeGpu::new(
                device,
                queue,
                &self.mesh,
                self.map_size[0],
                self.map_size[1],
                &self.map_rgba,
                self.map_version,
                self.target_format,
            ));
        }
        if let Some(gpu) = resources.get_mut::<GlobeGpu>() {
            if gpu.image_version != self.map_version {
                upload_map(
                    queue,
                    &gpu.texture,
                    self.map_size[0],
                    self.map_size[1],
                    &self.map_rgba,
                );
                gpu.image_version = self.map_version;
            }
            let camera = CameraUniform {
                angles: [
                    self.center_lon_deg.to_radians() as f32,
                    self.center_lat_deg.to_radians() as f32,
                    0.0,
                    0.0,
                ],
                shape: [
                    self.vertical_exaggeration,
                    DEFAULT_RADIUS_M,
                    GLOBE_SCALE,
                    0.0,
                ],
            };
            queue.write_buffer(&gpu.uniform, 0, bytemuck::bytes_of(&camera));
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let Some(gpu) = resources.get::<GlobeGpu>() else {
            return;
        };
        render_pass.set_pipeline(&gpu.pipeline);
        render_pass.set_bind_group(0, &gpu.bindings, &[]);
        render_pass.set_vertex_buffer(0, gpu.vertices.slice(..));
        render_pass.set_index_buffer(gpu.indices.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..gpu.index_count, 0, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Grid;
    use std::path::PathBuf;

    #[test]
    fn native_mesh_is_displaced_and_has_valid_indices() {
        let mut w = World::empty(PathBuf::new());
        w.width = 64;
        w.height = 32;
        w.elevation = Some(Grid::new(64, 32, 0.0));
        w.elevation.as_mut().unwrap().cells[16 * 64 + 32] = 4500.0;
        let mesh = GlobeMesh::from_world(&w);
        assert_eq!(
            mesh.vertices.len(),
            (mesh.lon_segments + 1) * (mesh.lat_segments + 1)
        );
        assert_eq!(
            mesh.triangle_count(),
            mesh.lon_segments * mesh.lat_segments * 2
        );
        assert!(mesh
            .indices
            .iter()
            .all(|i| (*i as usize) < mesh.vertices.len()));
        assert!(mesh.vertices.iter().any(|v| v.elevation_m > 0.0));
    }

    #[test]
    fn longitude_wrap_and_latitude_orientation_are_correct() {
        let w = World::empty(PathBuf::new());
        let m = GlobeMesh::from_world(&w);
        let first = m.vertices.first().unwrap();
        let last = m.vertices.last().unwrap();
        assert!((first.lon_lat[0] - std::f32::consts::PI).abs() < 1e-5);
        assert!((first.lon_lat[1] + std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert!((last.lon_lat[0] + std::f32::consts::PI).abs() < 1e-5);
        assert!((last.lon_lat[1] - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert!((first.uv[0] - 0.0).abs() < 1e-5);
        assert!((last.uv[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn globe_wgsl_shader_parses_and_validates_headlessly() {
        let module = naga::front::wgsl::parse_str(include_str!("globe3d.wgsl"))
            .expect("globe WGSL must parse");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("globe WGSL must validate without a GPU");
    }
}
