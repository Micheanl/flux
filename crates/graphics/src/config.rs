use wgpu::{Backends, Features, Limits, PowerPreference};

#[derive(Clone, Debug)]
pub struct GpuConfig {
    pub backends: Backends,
    pub power_preference: PowerPreference,
    pub features: Features,
    pub limits: Limits,
    pub label: &'static str,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            backends: Backends::VULKAN,
            power_preference: PowerPreference::HighPerformance,
            features: Features::empty(),
            limits: Limits::default(),
            label: "flux",
        }
    }
}

impl GpuConfig {
    pub(crate) fn backend_chain(&self) -> Vec<Backends> {
        let mut chain = vec![self.backends];
        let remaining = Backends::all() & !self.backends;
        if !remaining.is_empty() {
            chain.push(remaining);
        }
        chain
    }
}
