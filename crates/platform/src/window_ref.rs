use std::sync::Arc;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use winit::window::{CursorGrabMode, Window};

#[derive(Clone)]
pub struct WindowRef(Arc<Window>);

impl WindowRef {
    pub fn new(window: Arc<Window>) -> Self {
        Self(window)
    }

    pub fn size(&self) -> (u32, u32) {
        let size = self.0.inner_size();
        (size.width, size.height)
    }

    pub fn request_redraw(&self) {
        self.0.request_redraw();
    }

    pub fn set_title(&self, title: &str) {
        self.0.set_title(title);
    }

    pub fn set_cursor_visible(&self, visible: bool) {
        self.0.set_cursor_visible(visible);
    }

    pub fn set_mouse_grab(&self, grab: bool) -> bool {
        if grab {
            self.0.set_cursor_grab(CursorGrabMode::Locked).is_ok()
                || self.0.set_cursor_grab(CursorGrabMode::Confined).is_ok()
        } else {
            self.0.set_cursor_grab(CursorGrabMode::None).is_ok()
        }
    }

    pub fn id(&self) -> winit::window::WindowId {
        self.0.id()
    }
}

impl HasWindowHandle for WindowRef {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.0.window_handle()
    }
}

impl HasDisplayHandle for WindowRef {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.0.display_handle()
    }
}
