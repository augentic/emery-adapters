//! The dev binary's lazily connected live [`Model`] backend.
//! `SPECIFY_EVAL_MODEL` overrides the model id when the caller leaves `Request.model` unset.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use omnia::Backend as _;
use omnia_guest::Model;
use omnia_guest::model::{Error, Reply, Request};

use crate::native::Native;

/// Lazily connected cursor backend rooted at the project directory.
#[derive(Clone)]
pub struct DevModel {
    root: PathBuf,
    model: Option<String>,
    cell: Arc<tokio::sync::OnceCell<Native<omnia_cursor::Client>>>,
}

impl fmt::Debug for DevModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DevModel")
    }
}

impl DevModel {
    /// A lazily connected cursor backend rooted at `project_dir`.
    #[must_use]
    pub fn new(project_dir: &Path) -> Self {
        Self {
            root: project_dir.to_path_buf(),
            model: std::env::var("SPECIFY_EVAL_MODEL").ok().filter(|id| !id.trim().is_empty()),
            cell: Arc::new(tokio::sync::OnceCell::new()),
        }
    }
}

impl Model for DevModel {
    async fn create(&self, mut request: Request) -> Result<Reply, Error> {
        let native = self
            .cell
            .get_or_try_init(|| async {
                let client = omnia_cursor::Client::connect().await?;
                Ok::<_, anyhow::Error>(Native::new(client, self.root.clone()))
            })
            .await
            .map_err(|err| {
                Error::Backend(format!(
                    "cursor-agent backend unavailable: {err:#}; install cursor-agent, \
                     then `cursor-agent login` or export CURSOR_API_KEY (command-mode \
                     credentials, not the IDE login `cursor-agent status` reports)"
                ))
            })?;
        // A guest-supplied model id always wins over the driver override.
        if request.model.is_none() {
            request.model = self.model.clone();
        }
        native.create(request).await
    }
}
