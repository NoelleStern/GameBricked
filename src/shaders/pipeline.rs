//! 
//! Shader pipeline setup:
//! 
//!     WGPU shaders demand a lot of code overhead around them to work.
//!     I made some unified bindings:
//!         0 - Globals
//!         1 - Input texture
//!         2 - Input texture sampler
//! 


use std::sync::Arc;
use eframe::{egui::{self, ColorImage}, egui_wgpu::{self, CallbackTrait}, wgpu::{self, util::DeviceExt}};

use crate::{emu::rendering::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH}, shaders::wrapper::WgslWrapper};


/// Timing in milliseconds between frames
/// Used for converting milliseconds to frames
const FRAME_MS: f32 = 1.0 / 60.0;


#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Globals {
    /// Output resolution
    pub resolution: [f32; 2],
    /// Time counter in milliseconds
    pub time: f32,
    /// Frame counter
    pub frame: u32,
}

#[derive(Clone)]
pub struct ShaderEntry {
    /// Shader name
    pub name: String,
    /// Associated pipeline
    pipeline: Arc<wgpu::RenderPipeline>
}
impl ShaderEntry {
    pub fn new(name: String, pipeline: wgpu::RenderPipeline) -> Self {
        Self { 
            name,
            pipeline: Arc::new(pipeline)
        }
    }
}

pub struct ShaderState {
    /// Current shader id (0 is None, 1 indexes 0)
    pub shader_id: usize,
    /// Each shader has a different Pipeline
    shader_entries: Vec<ShaderEntry>,
    /// Shared render state
    render_resources: RenderResources,
}
impl ShaderState {
    pub fn new(rs: &eframe::egui_wgpu::RenderState) -> Self {
        Self {
            render_resources: RenderResources::new(rs),
            shader_id: 0, shader_entries: vec![],
        }
    }

    pub fn update(&mut self, ui: &egui::Ui, rect: egui::Rect, image: &ColorImage) {
        self.render_resources.update(ui, rect, image);
    }

    // Shader entry stuff
    pub fn get_entry_len(&self) -> usize { self.shader_entries.len() }
    pub fn get_entry(&self, entry_id: usize) -> &ShaderEntry { &self.shader_entries[entry_id - 1] }
    pub fn get_current_entry(&self) -> &ShaderEntry { &self.shader_entries[self.shader_id - 1] }
    pub fn add_shader_entry(&mut self, shader: &str, name: &str) {
        self.shader_entries.push( self.render_resources.create_shader_entry(shader, name) );
    }

    pub fn get_shader_callback(&self) -> ShaderCallback {
        ShaderCallback::new(self.get_current_entry().pipeline.clone())
    }
}

#[allow(dead_code)]
pub struct RenderResources {
    // Device info
        /// Command queue
        pub queue: wgpu::Queue,
        /// Graphics device
        pub device: wgpu::Device,
        /// Texture format
        pub format: wgpu::TextureFormat,
    // Shared pipeline bindings
        /// Shared bind group layout
        pub bind_group_layout: wgpu::BindGroupLayout,
        /// Shared bind group
        pub bind_group: wgpu::BindGroup, 
        /// Shared pipeline layout 
        pub pipeline_layout: wgpu::PipelineLayout,
        /// Texture view
        pub texture_view: wgpu::TextureView,
        /// Texture sampler
        pub sampler: wgpu::Sampler,
        /// Uniform buffer
        pub uniform_buffer: wgpu::Buffer,
        /// Shader uniforms
        pub uniform_globals: Globals,
}
impl RenderResources {
    pub fn new(rs: &eframe::egui_wgpu::RenderState) -> RenderResources {
        let queue = &rs.queue;
        let device = &rs.device;
        let rgba: Vec<u8> = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];

        let uniform_globals = Globals {
            resolution: [SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32],
            time: 0.0, frame: 0,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Shared uniforms"),
                contents: bytemuck::bytes_of(&uniform_globals),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            },
        );

        let texture_view = Self::create_texture_view_from_pixels(
            device, queue, &rgba, SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shared texture sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("Shared bind group layout"),
                entries: &[
                    // Uniforms
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Texture
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
                    // Sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            }
        );

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shared bind group"),
            layout: &bind_group_layout,
            entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding()                 }, // Binding 1: Globals
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&texture_view)  }, // Binding 1: Texture
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler)           }, // Binding 2: Sampler
                ],
            },
        );

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Custom standardized pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        // Add bind_group to resources
        rs.renderer
            .write() // Uses an RwLock/Mutex under the hood
            .callback_resources
            .insert(bind_group.clone());

        RenderResources {
            queue: queue.clone(),
            device: device.clone(),
            format: rs.target_format,
            bind_group_layout, bind_group,
            pipeline_layout,
            texture_view, sampler,
            uniform_buffer, uniform_globals,
        }
    }

    fn create_texture_view_from_pixels(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rgba: &[u8], // Raw RGBA
        width: u32,
        height: u32,
    ) -> wgpu::TextureView {
        let texture_size = wgpu::Extent3d {
            width, height,
            depth_or_array_layers: 1, // Your usual 2D image has a depth of 1
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Input Source Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm, // Standard RGBA
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfoBase {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4*width),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn create_shader_entry(&mut self, shader: &str, name: &str) -> ShaderEntry {
        let shader = self.device.create_shader_module(
            wgpu::ShaderModuleDescriptor {
                label: Some(name),
                source: wgpu::ShaderSource::Wgsl(
                    WgslWrapper::wrap(shader).into()
                ),
            },
        );

        let pipeline =
            self.device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("Custom standardized pipeline"),
                    layout: Some(&self.pipeline_layout),
                    // Vertex
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: Default::default(),
                    },
                    // Fragment
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_main"),
                        targets: &[
                            Some(
                                wgpu::ColorTargetState {
                                    format: self.format,
                                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                                    write_mask: wgpu::ColorWrites::ALL,
                                },
                            )
                        ],
                        compilation_options: Default::default(),
                    }),
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    multiview_mask: None,
                    cache: None,
                },
            );

        ShaderEntry::new(
            name.to_string(),
            pipeline
        )
    }

    pub fn update(&mut self, ui: &egui::Ui, rect: egui::Rect, image: &ColorImage) {
        let dt = ui.ctx().input(|i| i.stable_dt);

        let pixels_per_point = ui.ctx().pixels_per_point();
        let physical_width = rect.width() * pixels_per_point;
        let physical_height = rect.height() * pixels_per_point;

        self.uniform_globals.time += dt;
        self.uniform_globals.frame = (self.uniform_globals.time / FRAME_MS).floor() as u32;
        self.uniform_globals.resolution = [physical_width, physical_height];

        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&self.uniform_globals),
        );
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: self.texture_view.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image.pixels.iter()
                    .flat_map(|color| [color.r(), color.g(), color.b(), color.a()])
                    .collect::<Vec<u8>>(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4*image.width() as u32), 
                rows_per_image: Some(image.height() as u32),
            },
            wgpu::Extent3d {
                width: image.width() as u32, height: image.height() as u32, depth_or_array_layers: 1,
            }
        );
    }
}

pub struct ShaderCallback {
    pipeline: Arc<wgpu::RenderPipeline>,
}
impl ShaderCallback {
    pub fn new(pipeline: Arc<wgpu::RenderPipeline>) -> Self { 
        Self { pipeline }
    }
}
impl CallbackTrait for ShaderCallback {
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let bind_group = resources.get::<wgpu::BindGroup>().unwrap();
        
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}