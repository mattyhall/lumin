use axum::http::header;
use axum::response::IntoResponse;
use std::collections::HashMap;
use std::error::Error;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync;
use tracing::{debug, info, instrument};
use rayon::prelude::*;

pub const EXTENSIONS: &[&str] = &["css", "html", "jpg", "jpeg", "woff2"];

#[derive(Clone)]
pub struct Resource {
    path: PathBuf,
    contents: Vec<u8>,
}

impl Resource {
    fn content_type(&self) -> String {
        mime_guess::from_path(&self.path)
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

trait ResourceProcessor {
    fn matches(path: &Path) -> bool;
    fn process(path: &Path) -> Result<Resource, Box<dyn Error>>;
}

struct StaticProcessor {}
impl ResourceProcessor for StaticProcessor {
    fn matches(path: &Path) -> bool {
        path.extension()
            .map(|e| EXTENSIONS.iter().any(|wanted| *wanted == e))
            .unwrap_or(false)
    }

    #[instrument]
    fn process(path: &Path) -> Result<Resource, Box<dyn Error>> {
        info!("statically processing");

        let mut buf = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut buf)?;

        Ok(Resource {
            path: path.to_owned(),
            contents: buf,
        })
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

pub fn find_and_process<P: AsRef<Path>>(base: P) -> Result<Store, Box<dyn Error>> {
    let start = std::time::Instant::now();

    info!("rebuilding");

    let mut paths = Vec::new();
    let base = base.as_ref();

    walk(base, &mut paths)?;

    let store = Store::default();

    paths
        .into_par_iter()
        .try_for_each(|path| -> Result<(), String> {
            let mut store = store.clone();
            if !StaticProcessor::matches(&path) {
                return Ok(());
            }

            let short_path = path.strip_prefix(base).map_err(|e| e.to_string())?;
            store.put(short_path, StaticProcessor::process(&path).map_err(|e| e.to_string())?);

            Ok(())
        })?;

    info!(elapsed=?start.elapsed(), "rebuilding finished");

    Ok(store)
}
