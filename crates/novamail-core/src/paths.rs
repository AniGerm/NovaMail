use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub blobs_dir: PathBuf,
}

impl AppPaths {
    pub fn default_paths() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("novamail");
        Self::from_data_dir(data_dir)
    }

    pub fn from_data_dir(data_dir: PathBuf) -> Self {
        let db_path = data_dir.join("novamail.db");
        let blobs_dir = data_dir.join("blobs");
        Self {
            data_dir,
            db_path,
            blobs_dir,
        }
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.blobs_dir)?;
        Ok(())
    }
}
