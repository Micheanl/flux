use foundation::FrameTime;
use platform::{Flow, Input, RunError};

use crate::config::EngineConfig;
use crate::engine::Engine;

pub trait App: Sized + 'static {
    fn setup(engine: &mut Engine) -> Self;

    fn update(&mut self, engine: &mut Engine, input: &Input, time: FrameTime) -> Flow;
}

struct Runtime<A: App> {
    config: EngineConfig,
    engine: Option<Engine>,
    app: Option<A>,
}

impl<A: App> Runtime<A> {
    fn new(config: EngineConfig) -> Self {
        Self {
            config,
            engine: None,
            app: None,
        }
    }
}

impl<A: App> platform::Application for Runtime<A> {
    fn on_start(&mut self, window: platform::WindowRef) -> Result<(), RunError> {
        let mut engine = Engine::new(&self.config, window)
            .map_err(|error| RunError::Application(error.to_string()))?;
        let app = A::setup(&mut engine);
        self.engine = Some(engine);
        self.app = Some(app);
        Ok(())
    }

    fn on_frame(&mut self, input: &Input, time: FrameTime) -> Flow {
        let (Some(engine), Some(app)) = (self.engine.as_mut(), self.app.as_mut()) else {
            return Flow::Exit;
        };
        engine.update_camera(input, time.delta);
        if app.update(engine, input, time) == Flow::Exit {
            return Flow::Exit;
        }
        engine.advance_animations(time.delta);
        engine.render_frame()
    }

    fn on_resize(&mut self, width: u32, height: u32) {
        if let Some(engine) = self.engine.as_mut() {
            engine.resize(width, height);
        }
    }
}

pub fn run<A: App>(config: EngineConfig) -> Result<(), RunError> {
    let window_config = config.window.clone();
    platform::run(window_config, Runtime::<A>::new(config))
}
