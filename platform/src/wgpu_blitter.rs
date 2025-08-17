//! GPU-accelerated blitter used by the desktop simulator.
//!
//! `WgpuBlitter` implements the [`Blitter`](crate::blit::Blitter) trait using
//! the [`wgpu`](https://github.com/gfx-rs/wgpu) graphics API. Pixel operations
//! such as filling, blitting and alpha blending are executed on the GPU by
//! uploading surfaces to textures and rendering through small shader programs.
//! Results are read back into the caller provided buffers so the remainder of
//! the simulator can operate on plain memory.

use crate::blit::{BlitCaps, Blitter, PixelFmt, Rect, Surface};
use alloc::vec;
use pollster::block_on;

/// Render based blitter that executes transfers on the GPU.
pub struct WgpuBlitter {
    device: wgpu::Device,
    queue: wgpu::Queue,
    sampler: wgpu::Sampler,
    tex_layout: wgpu::BindGroupLayout,
    fill_layout: wgpu::BindGroupLayout,
    blit_pipeline: wgpu::RenderPipeline,
    blend_pipeline: wgpu::RenderPipeline,
    fill_pipeline: wgpu::RenderPipeline,
}

// WGSL shader drawing a textured quad.
const TEXTURE_SHADER: &str = r#"
@group(0) @binding(0) var u_texture: texture_2d<f32>;
@group(0) @binding(1) var u_sampler: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let p = positions[idx];
    var out: VsOut;
    out.pos = vec4<f32>(p * 2.0 - vec2<f32>(1.0, 1.0), 0.0, 1.0);
    out.uv = p;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(u_texture, u_sampler, in.uv);
}
"#;

// WGSL shader that fills with a solid colour.
const FILL_SHADER: &str = r#"
@group(0) @binding(0) var<uniform> u_color: vec4<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let p = positions[idx];
    var out: VsOut;
    out.pos = vec4<f32>(p * 2.0 - vec2<f32>(1.0, 1.0), 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(_in: VsOut) -> @location(0) vec4<f32> {
    return u_color;
}
"#;

impl WgpuBlitter {
    /// Create a new GPU blitter.
    pub fn new() -> Self {
        let instance = wgpu::Instance::default();
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("no suitable GPU adapters");
        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
            },
            None,
        ))
        .expect("device creation failed");

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        // Layout sampling from a texture.
        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture-layout"),
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
            ],
        });

        // Layout containing a uniform colour buffer.
        let fill_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fill-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let copy_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("copy-shader"),
            source: wgpu::ShaderSource::Wgsl(TEXTURE_SHADER.into()),
        });
        let fill_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fill-shader"),
            source: wgpu::ShaderSource::Wgsl(FILL_SHADER.into()),
        });

        let tex_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tex-pipeline-layout"),
            bind_group_layouts: &[&tex_layout],
            push_constant_ranges: &[],
        });

        let fill_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fill-pipeline-layout"),
            bind_group_layouts: &[&fill_layout],
            push_constant_ranges: &[],
        });

        let color_state = wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba8Unorm,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        };

        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit-pipeline"),
            layout: Some(&tex_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &copy_shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &copy_shader,
                entry_point: "fs_main",
                targets: &[Some(color_state.clone())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let blend_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blend-pipeline"),
            layout: Some(&tex_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &copy_shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &copy_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    ..color_state
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let fill_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fill-pipeline"),
            layout: Some(&fill_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &fill_shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &fill_shader,
                entry_point: "fs_main",
                targets: &[Some(color_state)],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            device,
            queue,
            sampler,
            tex_layout,
            fill_layout,
            blit_pipeline,
            blend_pipeline,
            fill_pipeline,
        }
    }

    // Upload an entire surface into a GPU texture.
    fn upload_surface(&self, surf: &Surface) -> wgpu::Texture {
        let extent = wgpu::Extent3d {
            width: surf.width,
            height: surf.height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("surface"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let row_bytes = (surf.width as usize) * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        if surf.stride == row_bytes && row_bytes % align == 0 {
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                surf.buf,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes as u32),
                    rows_per_image: Some(surf.height),
                },
                extent,
            );
        } else {
            let padded = ((row_bytes + align - 1) / align) * align;
            let mut tmp = vec![0u8; padded * surf.height as usize];
            for y in 0..surf.height as usize {
                let src_off = y * surf.stride;
                let dst_off = y * padded;
                tmp[dst_off..dst_off + row_bytes]
                    .copy_from_slice(&surf.buf[src_off..src_off + row_bytes]);
            }
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &tmp,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded as u32),
                    rows_per_image: Some(surf.height),
                },
                extent,
            );
        }

        texture
    }

    // Upload a rectangular region from a surface.
    fn upload_area(&self, surf: &Surface, area: Rect) -> wgpu::Texture {
        let extent = wgpu::Extent3d {
            width: area.w,
            height: area.h,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tile"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let bpp = 4usize;
        let row_bytes = area.w as usize * bpp;
        let mut data = vec![0u8; row_bytes * area.h as usize];
        for y in 0..area.h as usize {
            let src_off = (area.y as usize + y) * surf.stride + area.x as usize * bpp;
            let dst_off = y * row_bytes;
            data[dst_off..dst_off + row_bytes]
                .copy_from_slice(&surf.buf[src_off..src_off + row_bytes]);
        }
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        if row_bytes % align == 0 {
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &data,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes as u32),
                    rows_per_image: Some(area.h),
                },
                extent,
            );
        } else {
            let padded = ((row_bytes + align - 1) / align) * align;
            let mut tmp = vec![0u8; padded * area.h as usize];
            for y in 0..area.h as usize {
                let src_off = y * row_bytes;
                let dst_off = y * padded;
                tmp[dst_off..dst_off + row_bytes]
                    .copy_from_slice(&data[src_off..src_off + row_bytes]);
            }
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &tmp,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded as u32),
                    rows_per_image: Some(area.h),
                },
                extent,
            );
        }
        texture
    }

    // Read a texture back into a CPU surface.
    fn download_surface(&self, tex: &wgpu::Texture, surf: &mut Surface) {
        let row_bytes = surf.width as usize * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let padded = ((row_bytes + align - 1) / align) * align;
        let size = padded * surf.height as usize;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: size as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded as u32),
                    rows_per_image: Some(surf.height),
                },
            },
            wgpu::Extent3d {
                width: surf.width,
                height: surf.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::Maintain::Wait);
        let data = slice.get_mapped_range();
        for y in 0..surf.height as usize {
            let src_off = y * padded;
            let dst_off = y * surf.stride;
            surf.buf[dst_off..dst_off + row_bytes]
                .copy_from_slice(&data[src_off..src_off + row_bytes]);
        }
        drop(data);
        buffer.unmap();
    }
}

impl Blitter for WgpuBlitter {
    fn caps(&self) -> BlitCaps {
        BlitCaps::FILL | BlitCaps::BLIT | BlitCaps::BLEND
    }

    fn fill(&mut self, dst: &mut Surface, area: Rect, color: u32) {
        if dst.format != PixelFmt::Argb8888 {
            return;
        }
        let dst_tex = self.upload_surface(dst);
        let color_vec = [
            ((color >> 16) & 0xff) as f32 / 255.0,
            ((color >> 8) & 0xff) as f32 / 255.0,
            (color & 0xff) as f32 / 255.0,
            ((color >> 24) & 0xff) as f32 / 255.0,
        ];
        let color_bytes =
            unsafe { core::slice::from_raw_parts(color_vec.as_ptr() as *const u8, 16) };
        let color_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("color"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&color_buf, 0, color_bytes);

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.fill_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: color_buf.as_entire_binding(),
            }],
            label: Some("fill-bind"),
        });

        let view = dst_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fill-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.fill_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_viewport(0.0, 0.0, dst.width as f32, dst.height as f32, 0.0, 1.0);
            let x = area.x.max(0) as u32;
            let y = area.y.max(0) as u32;
            pass.set_scissor_rect(x, y, area.w, area.h);
            pass.draw(0..6, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.device.poll(wgpu::Maintain::Wait);
        self.download_surface(&dst_tex, dst);
    }

    fn blit(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32)) {
        if src.format != PixelFmt::Argb8888 || dst.format != PixelFmt::Argb8888 {
            return;
        }
        let src_tex = self.upload_area(src, src_area);
        let dst_tex = self.upload_surface(dst);
        let src_view = src_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.tex_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
            label: Some("blit-bind"),
        });
        let dst_view = dst_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &dst_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.set_viewport(
                dst_pos.0 as f32,
                dst_pos.1 as f32,
                src_area.w as f32,
                src_area.h as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(
                dst_pos.0.max(0) as u32,
                dst_pos.1.max(0) as u32,
                src_area.w,
                src_area.h,
            );
            pass.draw(0..6, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.device.poll(wgpu::Maintain::Wait);
        self.download_surface(&dst_tex, dst);
    }

    fn blend(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32)) {
        if src.format != PixelFmt::Argb8888 || dst.format != PixelFmt::Argb8888 {
            return;
        }
        let src_tex = self.upload_area(src, src_area);
        let dst_tex = self.upload_surface(dst);
        let src_view = src_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.tex_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
            label: Some("blend-bind"),
        });
        let dst_view = dst_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blend-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &dst_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.blend_pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.set_viewport(
                dst_pos.0 as f32,
                dst_pos.1 as f32,
                src_area.w as f32,
                src_area.h as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(
                dst_pos.0.max(0) as u32,
                dst_pos.1.max(0) as u32,
                src_area.w,
                src_area.h,
            );
            pass.draw(0..6, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.device.poll(wgpu::Maintain::Wait);
        self.download_surface(&dst_tex, dst);
    }
}
