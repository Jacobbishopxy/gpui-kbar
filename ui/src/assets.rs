use gpui::{Application, AssetSource, SharedString};
use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct WorkspaceAssets {
    root: PathBuf,
}

impl WorkspaceAssets {
    pub fn new() -> Self {
        Self {
            root: Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets"),
        }
    }

    fn resolve(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        }
    }
}

impl Default for WorkspaceAssets {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetSource for WorkspaceAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let resolved = self.resolve(path);
        match fs::read(&resolved) {
            Ok(bytes) => Ok(Some(Cow::Owned(bytes))),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let resolved = self.resolve(path);
        let entries = match fs::read_dir(&resolved) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(err) => return Err(err.into()),
        };

        let mut files = Vec::new();
        for entry in entries.flatten() {
            files.push(entry.path().to_string_lossy().into_owned().into());
        }
        Ok(files)
    }
}

pub fn application_with_assets() -> Application {
    Application::new().with_assets(WorkspaceAssets::new())
}
