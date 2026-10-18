use renderer::RendererConfig;

#[derive(Clone)]
pub struct EngineConfig {
    pub window: platform::WindowConfig,
    pub renderer: RendererConfig,
    pub show_fps: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            window: platform::WindowConfig::default(),
            renderer: RendererConfig::default(),
            show_fps: true,
        }
    }
}
