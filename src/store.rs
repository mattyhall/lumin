use axum::http::header;
use axum::response::IntoResponse;
use rayon::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync;
use tracing::{debug, info};

use crate::ResourceProcessor;

pub const EXTENSIONS: &[&str] = &[
    "css", "html", "jpg", "jpeg", "woff2", "liquid", "md", "markdown", "png", "svg", "webp",
];

#[derive(Clone, Default)]
pub enum URLPath {
    #[default]
    UseOriginalPath,

    Filepath(PathBuf),
    Absolute(String),
}

#[derive(Clone, Default)]
pub struct Resource {
    pub(crate) original_path: PathBuf,
    pub(crate) url_path: URLPath,
    pub(crate) contents: Vec<u8>,
}

impl Resource {
    fn path(&self) -> PathBuf {
        match &self.url_path {
            URLPath::UseOriginalPath => self.original_path.clone(),
            URLPath::Filepath(path) => path.clone(),
            URLPath::Absolute(s) => s.into(),
        }
    }

    fn url(&self, base: impl AsRef<Path>) -> Result<String, Box<dyn Error>> {
        let file_path = match &self.url_path {
            URLPath::UseOriginalPath => &self.original_path,
            URLPath::Filepath(path) => path,
            URLPath::Absolute(s) => return Ok(s.clone()),
        };

        Ok(file_path
            .strip_prefix(base)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .to_string())
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
    hm: sync::Arc<sync::Mutex<HashMap<String, Resource>>>,
}

impl Store {
    fn put(&mut self, path: String, resource: Resource) {
        if resource.contents.is_empty() {
            return;
        }

        info!(
            path,
            content_length = resource.contents.len(),
            "putting into store"
        );

        let mut hm = self.hm.lock().unwrap();
        hm.insert(path, resource);
    }

    pub fn get(&self, path: &str) -> Option<Resource> {
        let hm = self.hm.lock().unwrap();
        hm.get(path).cloned()
    }

    pub fn replace(&self, other: Store) {
        let mut other_handle = other.hm.lock().unwrap();
        let mut handle = self.hm.lock().unwrap();
        std::mem::swap(&mut *handle, &mut *other_handle)
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
                let url = resource.url(base).map_err(|e| e.to_string())?;
                store.put(url, resource);

                return Ok(());
            }

            Ok(())
        })?;

    let mut store = store;

    for processor in processors {
        let resources = processor.flush()?;
        if resources.is_empty() {
            continue;
        }

        info!(
            ?processor,
            count = resources.len(),
            "processor has extra resources"
        );

        for res in resources {
            let url = res.url(base)?;
            store.put(url, res);
        }
    }

    info!(elapsed=?start.elapsed(), "rebuilding finished");

    Ok(store)
}
