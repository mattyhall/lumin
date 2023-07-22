use axum::http::header;
use axum::response::IntoResponse;
use rayon::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync;
use tracing::{debug, info};

use crate::ResourceProcessor;

pub const EXTENSIONS: &[&str] = &["css", "html", "jpg", "jpeg", "woff2", "liquid", "md", "markdown"];

#[derive(Clone)]
pub struct Resource {
    pub(crate) original_path: PathBuf,
    pub(crate) renamed_path: Option<PathBuf>,
    pub(crate) contents: Vec<u8>,
}

impl Resource {
    fn path(&self) -> &Path {
        self.renamed_path.as_ref().unwrap_or(&self.original_path)
    }

    fn content_type(&self) -> String {
        mime_guess::from_path(self.path())
            .first_or_text_plain()
            .essence_str()
            .to_owned()
    }
}

impl IntoResponse for Resource {
    fn into_response(self) -> axum::response::Response {
        let typ = self.content_type();
        let res = ([(header::CONTENT_TYPE, typ)], self.contents);
        res.into_response()
    }
}

#[derive(Default, Clone)]
pub struct Store {
    hm: sync::Arc<sync::Mutex<HashMap<PathBuf, Resource>>>,
}

impl Store {
    fn put<P: Into<PathBuf>>(&mut self, path: P, resource: Resource) {
        let path = path.into();
        info!(
            ?path,
            content_length = resource.contents.len(),
            "putting into store"
        );

        let mut hm = self.hm.lock().unwrap();
        hm.insert(path, resource);
    }

    pub fn get<P: AsRef<Path>>(&self, path: P) -> Option<Resource> {
        let hm = self.hm.lock().unwrap();
        hm.get(path.as_ref()).cloned()
    }
}

fn walk(base: &Path, output: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in std::fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        if entry.metadata()?.is_dir() {
            walk(&path, output)?;
            continue;
        }

        match path.extension() {
            Some(ext) if EXTENSIONS.iter().any(|wanted| *wanted == ext) => {}
            _ => continue,
        }

        debug!(?path, "Found resource");

        output.push(path);
    }

    Ok(())
}

pub fn find_and_process<P: AsRef<Path>>(
    base: P,
    processors: &[&dyn ResourceProcessor],
) -> Result<Store, Box<dyn Error>> {
    let start = std::time::Instant::now();

    info!(base=?base.as_ref(), "rebuilding");

    let mut paths = Vec::new();
    let base = base.as_ref();

    walk(base, &mut paths)?;

    let store = Store::default();

    paths
        .into_par_iter()
        .try_for_each(|path| -> Result<(), String> {
            let mut store = store.clone();

            for processor in processors {
                if !processor.matches(&path) {
                    continue;
                }

                let resource = processor.process(&path).map_err(|e| e.to_string())?;
                let short_path = resource
                    .path()
                    .strip_prefix(base)
                    .map_err(|e| e.to_string())?
                    .to_owned();

                store.put(short_path, resource);

                return Ok(());
            }

            Ok(())
        })?;

    info!(elapsed=?start.elapsed(), "rebuilding finished");

    Ok(store)
}
