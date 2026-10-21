//! wgpu surface + egui renderer. The Apple screen is an egui texture (nearest filtering), so the
//! whole window is one egui pass.

use std::sync::Arc;
use winit::window::Window;

/// GPU objects shared by every window: one instance/adapter/device/queue per process. Each window
/// only owns a [`Gfx`] (surface + egui renderer).
pub struct GpuShared {
    // Field order is drop order; the instance goes last. Dropping it may terminate EGL, which needs
    // the display connection to still be alive (see `App::release_gpu`).
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: wgpu::Adapter,
    _instance: wgpu::Instance,
}

/// Per-window GPU state (surface, its configuration and the egui renderer).
pub struct Gfx {
    renderer: egui_wgpu::Renderer,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    capture: Option<std::path::PathBuf>,
    pub captured: bool,
    // Last: the shared context outlives every surface that was created from its instance.
    shared: Arc<GpuShared>,
}

impl Gfx {
    /// First window: creates the shared GPU context too (see [`Gfx::shared`]).
    pub fn new(window: Arc<Window>) -> Result<Gfx, String> {
        // PRIMARY = Vulkan/Metal/DX12. GL is only a fallback: initialising (and later terminating)
        // an EGL instance on Wayland+NVIDIA crashes, so avoid creating it when not needed.
        let backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY);
        let (instance, surface, adapter) = match Self::pick(&window, backends) {
            Some(x) => x,
            None if backends != wgpu::Backends::GL => {
                Self::pick(&window, wgpu::Backends::GL).ok_or("no suitable GPU adapter")?
            }
            None => return Err("no suitable GPU adapter".into()),
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .map_err(|e| e.to_string())?;
        let shared = Arc::new(GpuShared {
            device,
            queue,
            adapter,
            _instance: instance,
        });
        Self::with_surface(shared, surface, &window)
    }

    /// Another window on the same GPU context.
    pub fn for_window(shared: &Arc<GpuShared>, window: Arc<Window>) -> Result<Gfx, String> {
        let surface = shared
            ._instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;
        Self::with_surface(shared.clone(), surface, &window)
    }

    /// The shared context, to open further windows with [`Gfx::for_window`].
    pub fn shared(&self) -> Arc<GpuShared> {
        self.shared.clone()
    }

    fn with_surface(
        shared: Arc<GpuShared>,
        surface: wgpu::Surface<'static>,
        window: &Window,
    ) -> Result<Gfx, String> {
        let size = window.inner_size();
        let caps = surface.get_capabilities(&shared.adapter);
        if caps.formats.is_empty() || caps.alpha_modes.is_empty() {
            return Err("surface not supported by the adapter".into());
        }
        // egui wants a non-sRGB target
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else {
            caps.present_modes[0]
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&shared.device, &config);
        let renderer = egui_wgpu::Renderer::new(&shared.device, format, None, 1, false);
        Ok(Gfx {
            surface,
            config,
            renderer,
            capture: None,
            captured: false,
            shared,
        })
    }

    fn pick(
        window: &Arc<Window>,
        backends: wgpu::Backends,
    ) -> Option<(wgpu::Instance, wgpu::Surface<'static>, wgpu::Adapter)> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });
        let surface = instance.create_surface(window.clone()).ok()?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .or_else(|| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                force_fallback_adapter: true,
                ..Default::default()
            }))
        })?;
        eprintln!(
            "graphics: using {:?} ({})",
            adapter.get_info().backend,
            adapter.get_info().name
        );
        Some((instance, surface, adapter))
    }

    /// Saves the next presented frame (window contents, toolbar included) as a PNG.
    pub fn request_capture(&mut self, path: std::path::PathBuf) {
        self.capture = Some(path);
    }

    fn capture_texture(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
    ) -> wgpu::Buffer {
        let (w, h) = (self.config.width, self.config.height);
        let row = (w * 4).div_ceil(256) * 256;
        let buffer = self.shared.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture"),
            size: u64::from(row * h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        buffer
    }

    fn save_capture(&self, buffer: &wgpu::Buffer, path: &std::path::Path) -> Result<(), String> {
        let (w, h) = (self.config.width as usize, self.config.height as usize);
        let row = (w * 4).div_ceil(256) * 256;
        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| drop(tx.send(r)));
        self.shared.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let data = slice.get_mapped_range();
        let bgra = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let o = y * row + x * 4;
                let px = &data[o..o + 3];
                if bgra {
                    rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
                } else {
                    rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
        }
        drop(data);
        buffer.unmap();
        image::save_buffer(path, &rgba, w as u32, h as u32, image::ColorType::Rgba8)
            .map_err(|e| e.to_string())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.shared.device, &self.config);
        }
    }

    pub fn render(&mut self, ctx: &egui::Context, output: egui::FullOutput) {
        for (id, delta) in &output.textures_delta.set {
            self.renderer
                .update_texture(&self.shared.device, &self.shared.queue, *id, delta);
        }
        let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: output.pixels_per_point,
        };
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.shared.device, &self.config);
                return;
            }
            Err(_) => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.shared
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("egui"),
                });
        self.renderer.update_buffers(
            &self.shared.device,
            &self.shared.queue,
            &mut encoder,
            &jobs,
            &screen,
        );
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui"),
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
            });
            self.renderer
                .render(&mut pass.forget_lifetime(), &jobs, &screen);
        }
        let capture = self
            .capture
            .take()
            .map(|p| (self.capture_texture(&mut encoder, &frame.texture), p));
        self.shared.queue.submit(Some(encoder.finish()));
        if let Some((buffer, path)) = capture {
            match self.save_capture(&buffer, &path) {
                Ok(()) => self.captured = true,
                Err(e) => eprintln!("window capture failed: {e}"),
            }
        }
        frame.present();
        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }
}
