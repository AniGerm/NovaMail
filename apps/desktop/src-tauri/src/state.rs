use std::sync::Arc;

use novamail_core::{AppPaths, AppState};

pub struct DesktopState {
    pub app: Arc<AppState>,
}

impl DesktopState {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let paths = AppPaths::default_paths();
        let app = AppState::initialize(paths)?;
        Ok(Self {
            app: Arc::new(app),
        })
    }
}
