use std::sync::Arc;

use foundation::{FrameClock, FrameTime};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::input::Input;
use crate::window_ref::WindowRef;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Flow {
    Continue,
    Exit,
}

#[derive(Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Flux".to_string(),
            width: 1600,
            height: 900,
            resizable: true,
        }
    }
}

pub trait Application: Sized + 'static {
    fn on_start(&mut self, window: WindowRef) -> Result<(), RunError>;

    fn on_frame(&mut self, input: &Input, time: FrameTime) -> Flow;

    fn on_resize(&mut self, _width: u32, _height: u32) {}
}

#[derive(Debug)]
pub enum RunError {
    EventLoop(winit::error::EventLoopError),
    Window(winit::error::OsError),
    Application(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::EventLoop(error) => write!(f, "event loop error: {error}"),
            RunError::Window(error) => write!(f, "failed to create window: {error}"),
            RunError::Application(message) => write!(f, "application startup failed: {message}"),
        }
    }
}

impl std::error::Error for RunError {}

struct Runner<A: Application> {
    app: A,
    config: WindowConfig,
    window: Option<WindowRef>,
    input: Input,
    clock: FrameClock,
}

impl<A: Application> Runner<A> {
    fn new(config: WindowConfig, app: A) -> Self {
        Self {
            app,
            config,
            window: None,
            input: Input::default(),
            clock: FrameClock::new(),
        }
    }
}

impl<A: Application> ApplicationHandler for Runner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title(self.config.title.clone())
            .with_inner_size(LogicalSize::new(self.config.width, self.config.height))
            .with_resizable(self.config.resizable);

        let window = match event_loop.create_window(attributes) {
            Ok(window) => WindowRef::new(Arc::new(window)),
            Err(error) => {
                log::error!("failed to create window: {error}");
                event_loop.exit();
                return;
            }
        };

        if let Err(error) = self.app.on_start(window.clone()) {
            log::error!("{error}");
            event_loop.exit();
            return;
        }

        window.set_title(&self.config.title);
        window.request_redraw();
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                self.app.on_resize(size.width, size.height);
            }
            WindowEvent::Focused(focused) => self.input.on_focused(focused),
            WindowEvent::KeyboardInput { event, .. } => self.input.on_keyboard(event),
            WindowEvent::MouseInput { state, button, .. } => {
                self.input.on_mouse_button(button, state);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(position) => {
                        position.y as f32 / 60.0
                    }
                };
                self.input.on_mouse_wheel(amount);
            }
            WindowEvent::RedrawRequested => {
                let time = self.clock.tick();
                let flow = self.app.on_frame(&self.input, time);
                self.input.end_frame();
                match flow {
                    Flow::Continue => window.request_redraw(),
                    Flow::Exit => event_loop.exit(),
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.input
                .on_mouse_motion(foundation::Vec2::new(delta.0 as f32, delta.1 as f32));
        }
    }
}

pub fn run<A: Application>(config: WindowConfig, app: A) -> Result<(), RunError> {
    let event_loop = EventLoop::new().map_err(RunError::EventLoop)?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut runner = Runner::new(config, app);
    event_loop.run_app(&mut runner).map_err(RunError::EventLoop)
}
