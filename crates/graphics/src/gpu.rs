use std::sync::Arc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use wgpu::{
    Adapter, AdapterInfo, Backends, Device, Features, Instance, InstanceDescriptor, InstanceFlags,
    Limits, Queue, RequestAdapterOptions,
};

use crate::config::GpuConfig;
use crate::surface::Surface;

pub struct Gpu {
    pub instance: Instance,
    pub adapter: Adapter,
    pub device: Device,
    pub queue: Queue,
    pub info: AdapterInfo,
    pub features: Features,
    pub limits: Limits,
}

#[derive(Debug)]
pub enum GpuError {
    NoAdapter(String),
    Device(String),
    Surface(String),
    Unsupported(String),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::NoAdapter(message) => write!(f, "no GPU adapter available: {message}"),
            GpuError::Device(message) => write!(f, "failed to create GPU device: {message}"),
            GpuError::Surface(message) => write!(f, "failed to create surface: {message}"),
            GpuError::Unsupported(message) => write!(f, "unsupported: {message}"),
        }
    }
}

impl std::error::Error for GpuError {}

impl Gpu {
    pub fn new(config: &GpuConfig) -> Result<Self, GpuError> {
        let mut last_error = GpuError::NoAdapter("no compatible adapter".into());
        for backends in config.backend_chain() {
            let instance = create_instance(backends);
            let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
                power_preference: config.power_preference,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            }));
            match adapter {
                Ok(adapter) => return Self::open(instance, adapter, config),
                Err(_) => last_error = GpuError::NoAdapter(format!("backends {backends:?}")),
            }
        }
        Err(last_error)
    }

    pub fn new_for_window<W>(
        config: &GpuConfig,
        window: W,
        width: u32,
        height: u32,
    ) -> Result<(Self, Surface), GpuError>
    where
        W: HasWindowHandle + HasDisplayHandle + Clone + Send + Sync + 'static,
    {
        let mut last_error = GpuError::NoAdapter("no compatible adapter".into());
        for backends in config.backend_chain() {
            let instance = create_instance(backends);
            let raw_surface = match instance.create_surface(window.clone()) {
                Ok(surface) => surface,
                Err(error) => {
                    last_error = GpuError::Surface(error.to_string());
                    continue;
                }
            };
            let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
                power_preference: config.power_preference,
                compatible_surface: Some(&raw_surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            }));
            let Ok(adapter) = adapter else {
                last_error = GpuError::NoAdapter(format!("backends {backends:?}"));
                continue;
            };
            let gpu = Self::open(instance, adapter, config)?;
            let surface = Surface::new(raw_surface, &gpu, width, height)?;
            return Ok((gpu, surface));
        }
        Err(last_error)
    }

    pub fn create_surface<W>(&self, window: W, width: u32, height: u32) -> Result<Surface, GpuError>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        let surface = self
            .instance
            .create_surface(window)
            .map_err(|error| GpuError::Surface(error.to_string()))?;
        Surface::new(surface, self, width, height)
    }

    fn open(instance: Instance, adapter: Adapter, config: &GpuConfig) -> Result<Self, GpuError> {
        let available = adapter.features();
        let features = config.features & available;
        let missing = config.features - available;
        if !missing.is_empty() {
            log::warn!("GPU lacks features, disabling: {missing:?}");
        }

        let limits = config.limits.clone();
        let adapter_limits = adapter.limits();
        let descriptor = wgpu::DeviceDescriptor {
            label: Some(config.label),
            required_features: features,
            required_limits: limits,
            experimental_features: wgpu::ExperimentalFeatures::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        };

        let (device, queue) = pollster::block_on(adapter.request_device(&descriptor))
            .map_err(|error| GpuError::Device(error.to_string()))?;
        device.on_uncaptured_error(Arc::new(|error| log::error!("wgpu error: {error}")));

        let info = adapter.get_info();
        log::info!(
            "GPU: {} ({:?} / {:?})",
            info.name,
            info.backend,
            info.device_type
        );

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            info,
            features,
            limits: adapter_limits,
        })
    }
}

fn create_instance(backends: Backends) -> Instance {
    let mut descriptor = InstanceDescriptor::new_without_display_handle();
    descriptor.backends = backends;
    descriptor.flags = InstanceFlags::default().with_env();
    Instance::new(descriptor)
}
