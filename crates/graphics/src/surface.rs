use wgpu::{CurrentSurfaceTexture, SurfaceConfiguration, SurfaceTexture};

use crate::{Gpu, GpuError};

pub struct Surface {
    surface: wgpu::Surface<'static>,
    config: SurfaceConfiguration,
}

pub enum Acquisition {
    Ready(SurfaceTexture),
    Timeout,
    Occluded,
    Outdated,
    Lost,
    Validation,
}

impl Surface {
    pub(crate) fn new(
        surface: wgpu::Surface<'static>,
        gpu: &Gpu,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        let capabilities = surface.get_capabilities(&gpu.adapter);
        let Some(mut config) =
            surface.get_default_config(&gpu.adapter, width.max(1), height.max(1))
        else {
            return Err(GpuError::Unsupported(
                "surface is not compatible with this adapter".into(),
            ));
        };

        config.format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .unwrap_or(config.format);
        config.width = width.max(1);
        config.height = height.max(1);
        config.desired_maximum_frame_latency = 2;
        config.usage |= capabilities.usages & wgpu::TextureUsages::COPY_SRC;

        surface.configure(&gpu.device, &config);
        Ok(Self { surface, config })
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&gpu.device, &self.config);
    }

    pub fn acquire(&self) -> Acquisition {
        match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(texture)
            | CurrentSurfaceTexture::Suboptimal(texture) => Acquisition::Ready(texture),
            CurrentSurfaceTexture::Timeout => Acquisition::Timeout,
            CurrentSurfaceTexture::Occluded => Acquisition::Occluded,
            CurrentSurfaceTexture::Outdated => Acquisition::Outdated,
            CurrentSurfaceTexture::Lost => Acquisition::Lost,
            CurrentSurfaceTexture::Validation => Acquisition::Validation,
        }
    }

    pub fn present(&self, queue: &wgpu::Queue, texture: SurfaceTexture) {
        queue.present(texture);
    }

    pub fn reconfigure(&self, gpu: &Gpu) {
        self.surface.configure(&gpu.device, &self.config);
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }
}
