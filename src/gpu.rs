use std::collections::HashMap;
use std::fs;

use wgpu::*;
use winit::window::Window;

use crate::constants::{color, gpu as gpu_constants};
use crate::display::{DisplayItem, DrawBorder, DrawImage, DrawRect, DrawText};
use crate::image_loader;
use image::GenericImageView;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct RectVertex {
    pos: [f32; 2],
    color: [f32; 4],

    outer_min: [f32; 2],
    outer_max: [f32; 2],
    outer_radii: [f32; 4],

    inner_min: [f32; 2],
    inner_max: [f32; 2],
    inner_radii: [f32; 4],

    kind: f32, // 0 = fill, 1 = border ring
}
impl RectVertex {
    fn layout<'a>() -> VertexBufferLayout<'a> {
        use std::mem::size_of;
        let stride = size_of::<RectVertex>() as BufferAddress;

        VertexBufferLayout {
            array_stride: stride,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                // @location(0) pos
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x2,
                },
                // @location(1) color
                VertexAttribute {
                    offset: size_of::<[f32; 2]>() as BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Float32x4,
                },
                // @location(2) rect_min
                VertexAttribute {
                    offset: (size_of::<[f32; 2]>() + size_of::<[f32; 4]>()) as BufferAddress,
                    shader_location: 2,
                    format: VertexFormat::Float32x2,
                },
                // @location(3) outer_max
                VertexAttribute {
                    offset: (size_of::<[f32; 2]>() + size_of::<[f32; 4]>() + size_of::<[f32; 2]>())
                        as BufferAddress,
                    shader_location: 3,
                    format: VertexFormat::Float32x2,
                },
                // @location(4) outer_radii
                VertexAttribute {
                    offset: (size_of::<[f32; 2]>()
                        + size_of::<[f32; 4]>()
                        + size_of::<[f32; 2]>()
                        + size_of::<[f32; 2]>()) as BufferAddress,
                    shader_location: 4,
                    format: VertexFormat::Float32x4,
                },
                // @location(5) inner_min
                VertexAttribute {
                    offset: (size_of::<[f32; 2]>()
                        + size_of::<[f32; 4]>()
                        + size_of::<[f32; 2]>()
                        + size_of::<[f32; 2]>()
                        + size_of::<[f32; 4]>()) as BufferAddress,
                    shader_location: 5,
                    format: VertexFormat::Float32x2,
                },
                // @location(6) inner_max
                VertexAttribute {
                    offset: (size_of::<[f32; 2]>()
                        + size_of::<[f32; 4]>()
                        + size_of::<[f32; 2]>()
                        + size_of::<[f32; 2]>()
                        + size_of::<[f32; 4]>()
                        + size_of::<[f32; 2]>()) as BufferAddress,
                    shader_location: 6,
                    format: VertexFormat::Float32x2,
                },
                // @location(7) inner_radii
                VertexAttribute {
                    offset: (size_of::<[f32; 2]>()
                        + size_of::<[f32; 4]>()
                        + size_of::<[f32; 2]>()
                        + size_of::<[f32; 2]>()
                        + size_of::<[f32; 4]>()
                        + size_of::<[f32; 2]>()
                        + size_of::<[f32; 2]>()) as BufferAddress,
                    shader_location: 7,
                    format: VertexFormat::Float32x4,
                },
                // @location(8) kind
                VertexAttribute {
                    offset: (size_of::<[f32; 2]>()
                        + size_of::<[f32; 4]>()
                        + size_of::<[f32; 2]>()
                        + size_of::<[f32; 2]>()
                        + size_of::<[f32; 4]>()
                        + size_of::<[f32; 2]>()
                        + size_of::<[f32; 2]>()
                        + size_of::<[f32; 4]>()) as BufferAddress,
                    shader_location: 8,
                    format: VertexFormat::Float32,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}
impl TextVertex {
    fn layout<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<TextVertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x2,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Float32x2,
                },
                VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 2]>() * 2) as BufferAddress,
                    shader_location: 2,
                    format: VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ImageVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}
impl ImageVertex {
    fn layout<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<ImageVertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x2,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[derive(Clone, Copy)]
struct GlyphEntry {
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    w: u32,
    h: u32,
    xmin: i32,
    ymin: i32,
    advance: f32,
}

struct AtlasPacker {
    x: u32,
    y: u32,
    row_h: u32,
    size: u32,
}
impl AtlasPacker {
    fn new(size: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            row_h: 0,
            size,
        }
    }
    fn alloc(&mut self, w: u32, h: u32, pad: u32) -> Option<(u32, u32)> {
        let w = w + pad * 2;
        let h = h + pad * 2;

        if w > self.size || h > self.size {
            return None;
        }
        if self.x + w > self.size {
            self.x = 0;
            self.y += self.row_h;
            self.row_h = 0;
        }
        if self.y + h > self.size {
            return None;
        }
        let ox = self.x + pad;
        let oy = self.y + pad;

        self.x += w;
        self.row_h = self.row_h.max(h);

        Some((ox, oy))
    }
}

struct CachedImage {
    bind_group: BindGroup,
    _view: TextureView, // view を保持しておく（bind_group が参照するので寿命管理）
    _tex: Texture,      // texture も保持
}

enum PreparedDrawCommand {
    Rect { start: u32, count: u32 },
    Text { start: u32, count: u32 },
    Image { start: u32, count: u32, key: String },
}

fn display_item_fixed(item: &DisplayItem) -> bool {
    match item {
        DisplayItem::Rect(r) => r.fixed,
        DisplayItem::Border(b) => b.fixed,
        DisplayItem::Text(t) => t.fixed,
        DisplayItem::Image(im) => im.fixed,
    }
}

pub struct GPU<'a> {
    pub surface: Surface<'a>,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,

    // rect pipeline
    rect_pipeline: RenderPipeline,
    rect_vbuf: Buffer,
    rect_cap: usize,

    // text pipeline
    text_pipeline: RenderPipeline,
    text_vbuf: Buffer,
    text_cap: usize,

    // image pipeline
    image_pipeline: RenderPipeline,
    image_vbuf: Buffer,
    image_cap: usize,
    image_bgl: BindGroupLayout,
    image_sampler: Sampler,
    image_cache: HashMap<String, CachedImage>,

    // glyph atlas
    atlas_tex: Texture,
    #[allow(dead_code)]
    atlas_view: TextureView,
    #[allow(dead_code)]
    atlas_sampler: Sampler,
    atlas_bind_group: BindGroup,
    atlas_size: u32,
    packer: AtlasPacker,

    // font + glyph cache
    font: fontdue::Font,
    glyph_cache: HashMap<(char, u32), GlyphEntry>, // (char, size_px_rounded)
}

impl<'a> GPU<'a> {
    pub async fn new(window: &'a Window) -> Self {
        let size = window.inner_size();
        let instance = Instance::default();
        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: None,
                    required_features: Features::empty(),
                    required_limits: Limits::default(),
                },
                None,
            )
            .await
            .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(gpu_constants::MIN_SURFACE_SIZE_PX),
            height: size.height.max(gpu_constants::MIN_SURFACE_SIZE_PX),
            present_mode: PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: gpu_constants::MAX_FRAME_LATENCY,
        };
        surface.configure(&device, &config);

        // ---------- load font (Meiryo) ----------
        let font = load_meiryo_font();

        // ---------- rect pipeline ----------
        let rect_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("rect shader"),
            source: ShaderSource::Wgsl(include_str!("rect.wgsl").into()),
        });
        let rect_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("rect layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let rect_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("rect pipeline"),
            layout: Some(&rect_layout),
            vertex: VertexState {
                module: &rect_shader,
                entry_point: "vs_main",
                buffers: &[RectVertex::layout()],
            },
            fragment: Some(FragmentState {
                module: &rect_shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: config.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
        });

        let rect_cap = gpu_constants::INITIAL_RECT_VERTEX_CAPACITY;
        let rect_vbuf = device.create_buffer(&BufferDescriptor {
            label: Some("rect vbuf"),
            size: (rect_cap * std::mem::size_of::<RectVertex>()) as BufferAddress,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ---------- glyph atlas ----------
        let atlas_size = gpu_constants::GLYPH_ATLAS_SIZE_PX;
        let atlas_tex = device.create_texture(&TextureDescriptor {
            label: Some("glyph atlas"),
            size: Extent3d {
                width: atlas_size,
                height: atlas_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_tex.create_view(&TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("glyph sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let atlas_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("atlas bgl"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let atlas_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("atlas bind group"),
            layout: &atlas_bgl,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&atlas_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        // ---------- text pipeline ----------
        let text_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("text shader"),
            source: ShaderSource::Wgsl(include_str!("text.wgsl").into()),
        });
        let text_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("text layout"),
            bind_group_layouts: &[&atlas_bgl],
            push_constant_ranges: &[],
        });

        let text_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("text pipeline"),
            layout: Some(&text_layout),
            vertex: VertexState {
                module: &text_shader,
                entry_point: "vs_main",
                buffers: &[TextVertex::layout()],
            },
            fragment: Some(FragmentState {
                module: &text_shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: config.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
        });

        let text_cap = gpu_constants::INITIAL_TEXT_VERTEX_CAPACITY;
        let text_vbuf = device.create_buffer(&BufferDescriptor {
            label: Some("text vbuf"),
            size: (text_cap * std::mem::size_of::<TextVertex>()) as BufferAddress,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ---------- image sampler / bgl / pipeline ----------
        let image_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("image sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let image_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("image bgl"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let image_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("image shader"),
            source: ShaderSource::Wgsl(include_str!("image.wgsl").into()),
        });

        let image_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("image layout"),
            bind_group_layouts: &[&image_bgl],
            push_constant_ranges: &[],
        });

        let image_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("image pipeline"),
            layout: Some(&image_layout),
            vertex: VertexState {
                module: &image_shader,
                entry_point: "vs_main",
                buffers: &[ImageVertex::layout()],
            },
            fragment: Some(FragmentState {
                module: &image_shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: config.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
        });

        let image_cap = gpu_constants::INITIAL_IMAGE_VERTEX_CAPACITY;
        let image_vbuf = device.create_buffer(&BufferDescriptor {
            label: Some("image vbuf"),
            size: (image_cap * std::mem::size_of::<ImageVertex>()) as BufferAddress,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            surface,
            device,
            queue,
            config,

            rect_pipeline,
            rect_vbuf,
            rect_cap,

            text_pipeline,
            text_vbuf,
            text_cap,

            image_pipeline,
            image_vbuf,
            image_cap,
            image_bgl,
            image_sampler,
            image_cache: HashMap::new(),

            atlas_tex,
            atlas_view,
            atlas_sampler,
            atlas_bind_group,
            atlas_size,
            packer: AtlasPacker::new(atlas_size),

            font,
            glyph_cache: HashMap::new(),
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn render_items(&mut self, items: &[DisplayItem], scroll_y: f32) {
        // 先に image のキャッシュを作る（bind_group が必要）。
        for item in items {
            if let DisplayItem::Image(im) = item {
                let _ = self.get_or_upload_image(&im.key, &im.src);
            }
        }

        let mut rect_verts = Vec::<RectVertex>::new();
        let mut image_verts = Vec::<ImageVertex>::new();
        let mut text_verts = Vec::<TextVertex>::new();
        let mut normal_commands = Vec::<PreparedDrawCommand>::new();
        let mut fixed_commands = Vec::<PreparedDrawCommand>::new();

        for item in items {
            let commands = if display_item_fixed(item) {
                &mut fixed_commands
            } else {
                &mut normal_commands
            };

            match item {
                DisplayItem::Rect(r) => {
                    let start = rect_verts.len();
                    rect_verts.extend(self.rect_vertices(std::slice::from_ref(r), scroll_y));
                    let count = rect_verts.len() - start;
                    if count > 0 {
                        commands.push(PreparedDrawCommand::Rect {
                            start: start as u32,
                            count: count as u32,
                        });
                    }
                }
                DisplayItem::Border(b) => {
                    let start = rect_verts.len();
                    rect_verts.extend(self.border_vertices(std::slice::from_ref(b), scroll_y));
                    let count = rect_verts.len() - start;
                    if count > 0 {
                        commands.push(PreparedDrawCommand::Rect {
                            start: start as u32,
                            count: count as u32,
                        });
                    }
                }
                DisplayItem::Text(t) => {
                    let start = text_verts.len();
                    let verts = self.text_vertices(std::slice::from_ref(t), scroll_y);
                    text_verts.extend(verts);
                    let count = text_verts.len() - start;
                    if count > 0 {
                        commands.push(PreparedDrawCommand::Text {
                            start: start as u32,
                            count: count as u32,
                        });
                    }
                }
                DisplayItem::Image(im) => {
                    if !self.image_cache.contains_key(&im.key) {
                        continue;
                    }

                    let start = image_verts.len();
                    image_verts.extend(self.image_vertices(std::slice::from_ref(im), scroll_y));
                    let count = image_verts.len() - start;
                    if count > 0 {
                        commands.push(PreparedDrawCommand::Image {
                            start: start as u32,
                            count: count as u32,
                            key: im.key.clone(),
                        });
                    }
                }
            }
        }

        self.ensure_rect_capacity(rect_verts.len());
        self.ensure_image_capacity(image_verts.len());
        self.ensure_text_capacity(text_verts.len());

        if !rect_verts.is_empty() {
            self.queue
                .write_buffer(&self.rect_vbuf, 0, bytemuck::cast_slice(&rect_verts));
        }
        if !image_verts.is_empty() {
            self.queue
                .write_buffer(&self.image_vbuf, 0, bytemuck::cast_slice(&image_verts));
        }
        if !text_verts.is_empty() {
            self.queue
                .write_buffer(&self.text_vbuf, 0, bytemuck::cast_slice(&text_verts));
        }

        let output = match self.surface.get_current_texture() {
            Ok(o) => o,
            Err(_) => {
                self.surface.configure(&self.device, &self.config);
                self.surface.get_current_texture().unwrap()
            }
        };

        let view = output
            .texture
            .create_view(&TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(color::CLEAR_BACKGROUND),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            for command in normal_commands.iter().chain(fixed_commands.iter()) {
                match command {
                    PreparedDrawCommand::Rect { start, count } => {
                        pass.set_pipeline(&self.rect_pipeline);
                        pass.set_vertex_buffer(0, self.rect_vbuf.slice(..));
                        pass.draw(*start..(*start + *count), 0..1);
                    }
                    PreparedDrawCommand::Text { start, count } => {
                        pass.set_pipeline(&self.text_pipeline);
                        pass.set_bind_group(0, &self.atlas_bind_group, &[]);
                        pass.set_vertex_buffer(0, self.text_vbuf.slice(..));
                        pass.draw(*start..(*start + *count), 0..1);
                    }
                    PreparedDrawCommand::Image { start, count, key } => {
                        if let Some(cached) = self.image_cache.get(key) {
                            pass.set_pipeline(&self.image_pipeline);
                            pass.set_bind_group(0, &cached.bind_group, &[]);
                            pass.set_vertex_buffer(0, self.image_vbuf.slice(..));
                            pass.draw(*start..(*start + *count), 0..1);
                        } else {
                            eprintln!("[img] cache miss key={}", key);
                        }
                    }
                }
            }
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();
    }

    pub fn viewport_height(&self) -> f32 {
        self.config.height as f32
    }

    pub fn viewport_width(&self) -> f32 {
        self.config.width as f32
    }

    fn ensure_rect_capacity(&mut self, need: usize) {
        let need = need.max(1);
        if need > self.rect_cap {
            self.rect_cap = need.next_power_of_two();
            self.rect_vbuf = self.device.create_buffer(&BufferDescriptor {
                label: Some("rect vbuf resized"),
                size: (self.rect_cap * std::mem::size_of::<RectVertex>()) as BufferAddress,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
    }

    fn ensure_text_capacity(&mut self, need: usize) {
        let need = need.max(1);
        if need > self.text_cap {
            self.text_cap = need.next_power_of_two();
            self.text_vbuf = self.device.create_buffer(&BufferDescriptor {
                label: Some("text vbuf resized"),
                size: (self.text_cap * std::mem::size_of::<TextVertex>()) as BufferAddress,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
    }

    fn ensure_image_capacity(&mut self, need: usize) {
        let need = need.max(1);
        if need > self.image_cap {
            self.image_cap = need.next_power_of_two();
            self.image_vbuf = self.device.create_buffer(&BufferDescriptor {
                label: Some("image vbuf resized"),
                size: (self.image_cap * std::mem::size_of::<ImageVertex>()) as BufferAddress,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
    }

    fn rect_vertices(&self, rects: &[DrawRect], scroll_y: f32) -> Vec<RectVertex> {
        let mut out = Vec::with_capacity(rects.len() * gpu_constants::VERTICES_PER_QUAD);

        let w = self.config.width as f32;
        let h = self.config.height as f32;

        for r in rects {
            if r.w <= 0.0 || r.h <= 0.0 {
                continue;
            }

            let scroll = if r.fixed { 0.0 } else { scroll_y };
            let ry = r.y - scroll;

            if ry > h || (ry + r.h) < 0.0 {
                continue;
            }
            if r.x > w || (r.x + r.w) < 0.0 {
                continue;
            }

            // NDC 変換
            let x1 = (r.x / w) * 2.0 - 1.0;
            let y1 = 1.0 - (ry / h) * 2.0;
            let x2 = ((r.x + r.w) / w) * 2.0 - 1.0;
            let y2 = 1.0 - ((ry + r.h) / h) * 2.0;

            push_rect_vertices(
                &mut out,
                [x1, y1],
                [x2, y2],
                [r.x, ry],
                [r.x + r.w, ry + r.h],
                r.radius.normalize(r.w, r.h).as_array(),
                [r.x, ry],
                [r.x, ry],
                [0.0; 4],
                0.0,
                r.color,
            );
        }

        out
    }

    fn border_vertices(&self, borders: &[DrawBorder], scroll_y: f32) -> Vec<RectVertex> {
        let mut out = Vec::with_capacity(borders.len() * gpu_constants::VERTICES_PER_QUAD);

        let w = self.config.width as f32;
        let h = self.config.height as f32;

        for b in borders {
            if b.w <= 0.0 || b.h <= 0.0 {
                continue;
            }

            let scroll = if b.fixed { 0.0 } else { scroll_y };
            let ry = b.y - scroll;
            if ry > h || (ry + b.h) < 0.0 {
                continue;
            }
            if b.x > w || (b.x + b.w) < 0.0 {
                continue;
            }

            let x1 = (b.x / w) * 2.0 - 1.0;
            let y1 = 1.0 - (ry / h) * 2.0;
            let x2 = ((b.x + b.w) / w) * 2.0 - 1.0;
            let y2 = 1.0 - ((ry + b.h) / h) * 2.0;

            let outer = b.radius.normalize(b.w, b.h);
            let bw = b.border_width.max(0.0);

            let inner_w = (b.w - bw * 2.0).max(0.0);
            let inner_h = (b.h - bw * 2.0).max(0.0);
            let inner_min = [b.x + bw, ry + bw];
            let inner_max = [inner_min[0] + inner_w, inner_min[1] + inner_h];
            let inner = outer.inset_uniform(bw).normalize(inner_w, inner_h);

            let kind = if inner_w > 0.0 && inner_h > 0.0 {
                1.0
            } else {
                0.0
            };

            push_rect_vertices(
                &mut out,
                [x1, y1],
                [x2, y2],
                [b.x, ry],
                [b.x + b.w, ry + b.h],
                outer.as_array(),
                inner_min,
                inner_max,
                inner.as_array(),
                kind,
                b.color,
            );
        }

        out
    }

    fn image_vertices(&self, images: &[DrawImage], scroll_y: f32) -> Vec<ImageVertex> {
        let mut out = Vec::with_capacity(images.len() * gpu_constants::VERTICES_PER_QUAD);

        let w = self.config.width as f32;
        let h = self.config.height as f32;

        for im in images {
            if im.w <= 0.0 || im.h <= 0.0 {
                continue;
            }

            let scroll = if im.fixed { 0.0 } else { scroll_y };
            let ry = im.y - scroll;
            if ry > h || (ry + im.h) < 0.0 {
                continue;
            }
            if im.x > w || (im.x + im.w) < 0.0 {
                continue;
            }

            let x1 = (im.x / w) * 2.0 - 1.0;
            let y1 = 1.0 - (ry / h) * 2.0;
            let x2 = ((im.x + im.w) / w) * 2.0 - 1.0;
            let y2 = 1.0 - ((ry + im.h) / h) * 2.0;

            let u0 = 0.0;
            let v0 = 0.0;
            let u1 = 1.0;
            let v1 = 1.0;

            out.push(ImageVertex {
                pos: [x1, y1],
                uv: [u0, v0],
            });
            out.push(ImageVertex {
                pos: [x2, y1],
                uv: [u1, v0],
            });
            out.push(ImageVertex {
                pos: [x2, y2],
                uv: [u1, v1],
            });

            out.push(ImageVertex {
                pos: [x1, y1],
                uv: [u0, v0],
            });
            out.push(ImageVertex {
                pos: [x2, y2],
                uv: [u1, v1],
            });
            out.push(ImageVertex {
                pos: [x1, y2],
                uv: [u0, v1],
            });
        }

        out
    }

    fn text_vertices(&mut self, texts: &[DrawText], scroll_y: f32) -> Vec<TextVertex> {
        let mut out = Vec::new();

        let w = self.config.width as f32;
        let h = self.config.height as f32;

        for t in texts {
            let size_px = t.size_px.max(gpu_constants::MIN_TEXT_SIZE_PX);
            let key_size = size_px.round() as u32;

            let mut pen_x = t.x;
            let scroll = if t.fixed { 0.0 } else { scroll_y };
            let baseline_y = t.y - scroll;

            let top = t.hit.y - scroll;
            let bottom = top + t.hit.height;
            if bottom < 0.0 || top > h {
                continue;
            }

            for ch in t.text.chars() {
                if ch == '\n' {
                    pen_x = t.x;
                    continue;
                }

                let ge = self.get_or_upload_glyph(ch, key_size);

                let gx = pen_x + ge.xmin as f32;
                let gy = baseline_y - (ge.h as f32) - ge.ymin as f32;

                if gy > h || (gy + ge.h as f32) < 0.0 || gx > w || (gx + ge.w as f32) < 0.0 {
                    pen_x += ge.advance;
                    continue;
                }

                let x1 = (gx / w) * 2.0 - 1.0;
                let y1 = 1.0 - (gy / h) * 2.0;
                let x2 = ((gx + ge.w as f32) / w) * 2.0 - 1.0;
                let y2 = 1.0 - ((gy + ge.h as f32) / h) * 2.0;

                let u0 = ge.u0;
                let v0 = ge.v0;
                let u1 = ge.u1;
                let v1 = ge.v1;
                let c = t.color;

                out.push(TextVertex {
                    pos: [x1, y1],
                    uv: [u0, v0],
                    color: c,
                });
                out.push(TextVertex {
                    pos: [x2, y1],
                    uv: [u1, v0],
                    color: c,
                });
                out.push(TextVertex {
                    pos: [x2, y2],
                    uv: [u1, v1],
                    color: c,
                });

                out.push(TextVertex {
                    pos: [x1, y1],
                    uv: [u0, v0],
                    color: c,
                });
                out.push(TextVertex {
                    pos: [x2, y2],
                    uv: [u1, v1],
                    color: c,
                });
                out.push(TextVertex {
                    pos: [x1, y2],
                    uv: [u0, v1],
                    color: c,
                });

                pen_x += ge.advance;
            }
        }

        out
    }

    fn get_or_upload_glyph(&mut self, ch: char, size_px: u32) -> GlyphEntry {
        if let Some(e) = self.glyph_cache.get(&(ch, size_px)) {
            return *e;
        }

        let (metrics, bitmap) = self.font.rasterize(ch, size_px as f32);
        let gw = metrics.width as u32;
        let gh = metrics.height as u32;

        if gw == 0 || gh == 0 {
            let e = GlyphEntry {
                u0: 0.0,
                v0: 0.0,
                u1: 0.0,
                v1: 0.0,
                w: 0,
                h: 0,
                xmin: metrics.xmin,
                ymin: metrics.ymin,
                advance: metrics.advance_width,
            };
            self.glyph_cache.insert((ch, size_px), e);
            return e;
        }

        let (ax, ay) = self
            .packer
            .alloc(gw, gh, gpu_constants::GLYPH_ATLAS_PADDING_PX)
            .expect("atlas full (increase atlas size)");

        self.queue.write_texture(
            ImageCopyTexture {
                texture: &self.atlas_tex,
                mip_level: 0,
                origin: Origin3d { x: ax, y: ay, z: 0 },
                aspect: TextureAspect::All,
            },
            &bitmap,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(gw),
                rows_per_image: Some(gh),
            },
            Extent3d {
                width: gw,
                height: gh,
                depth_or_array_layers: 1,
            },
        );

        let u0 = ax as f32 / self.atlas_size as f32;
        let v0 = ay as f32 / self.atlas_size as f32;
        let u1 = (ax + gw) as f32 / self.atlas_size as f32;
        let v1 = (ay + gh) as f32 / self.atlas_size as f32;

        let e = GlyphEntry {
            u0,
            v0,
            u1,
            v1,
            w: gw,
            h: gh,
            xmin: metrics.xmin,
            ymin: metrics.ymin,
            advance: metrics.advance_width,
        };

        self.glyph_cache.insert((ch, size_px), e);
        e
    }

    fn get_or_upload_image(&mut self, key: &str, src: &str) -> Option<&CachedImage> {
        if self.image_cache.contains_key(key) {
            return self.image_cache.get(key);
        }

        // file:// / http(s):// 両対応（image_loader に委譲）
        let bytes = image_loader::load_image_bytes(src)?;
        let img = image::load_from_memory(&bytes).ok()?;
        let rgba = img.to_rgba8();
        let (iw, ih) = img.dimensions();

        if iw == 0 || ih == 0 {
            eprintln!("[img] decoded but zero-size: key={} src={}", key, src);
            return None;
        }

        let tex = self.device.create_texture(&TextureDescriptor {
            label: Some("img tex"),
            size: Extent3d {
                width: iw,
                height: ih,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            ImageCopyTexture {
                texture: &tex,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &rgba,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(iw * gpu_constants::RGBA_BYTES_PER_PIXEL),
                rows_per_image: Some(ih),
            },
            Extent3d {
                width: iw,
                height: ih,
                depth_or_array_layers: 1,
            },
        );

        let view = tex.create_view(&TextureViewDescriptor::default());

        let bg = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("img bind group"),
            layout: &self.image_bgl,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&self.image_sampler),
                },
            ],
        });

        self.image_cache.insert(
            key.to_string(),
            CachedImage {
                bind_group: bg,
                _view: view,
                _tex: tex,
            },
        );
        self.image_cache.get(key)
    }
}

// ----------------- font loader -----------------

fn load_meiryo_font() -> fontdue::Font {
    let candidates = [
        r"C:\Windows\Fonts\meiryo.ttc",
        r"C:\Windows\Fonts\meiryob.ttc",
        r"C:\Windows\Fonts\YuGothR.ttc",
        r"C:\Windows\Fonts\msgothic.ttc",
    ];

    for p in candidates {
        if let Ok(bytes) = fs::read(p) {
            let settings = fontdue::FontSettings {
                collection_index: gpu_constants::FONT_COLLECTION_INDEX,
                ..Default::default()
            };
            if let Ok(font) = fontdue::Font::from_bytes(bytes, settings) {
                println!("Font loaded: {}", p);
                return font;
            }
        }
    }

    panic!("日本語フォントが見つからない: C:\\Windows\\Fonts\\meiryo.ttc 等を確認して");
}

fn push_rect_vertices(
    out: &mut Vec<RectVertex>,
    ndc_min: [f32; 2],
    ndc_max: [f32; 2],
    outer_min: [f32; 2],
    outer_max: [f32; 2],
    outer_radii: [f32; 4],
    inner_min: [f32; 2],
    inner_max: [f32; 2],
    inner_radii: [f32; 4],
    kind: f32,
    color: [f32; 4],
) {
    let v = |px: f32, py: f32| RectVertex {
        pos: [px, py],
        color,
        outer_min,
        outer_max,
        outer_radii,
        inner_min,
        inner_max,
        inner_radii,
        kind,
    };

    out.push(v(ndc_min[0], ndc_min[1]));
    out.push(v(ndc_max[0], ndc_min[1]));
    out.push(v(ndc_max[0], ndc_max[1]));

    out.push(v(ndc_min[0], ndc_min[1]));
    out.push(v(ndc_max[0], ndc_max[1]));
    out.push(v(ndc_min[0], ndc_max[1]));
}
