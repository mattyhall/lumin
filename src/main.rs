use axum::routing::get;
use axum::{Extension, Router};
use std::collections::HashMap;
use std::error::Error;
use std::io::Read;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{debug, info, instrument};

const EXTENSIONS: &[&str] = &["css", "html", "jpg", "jpeg", "woff2"];

trait ResourceProcessor {
    fn matches(path: &Path) -> bool;
    fn process(path: &Path) -> Result<String, Box<dyn Error>>;
}

struct StaticProcessor {}
impl ResourceProcessor for StaticProcessor {
    fn matches(path: &Path) -> bool {
        path.extension()
            .map(|e| e == "css" || e == "html" || e == "jpg")
            .unwrap_or(false)
    }

    #[instrument]
    fn process(path: &Path) -> Result<String, Box<dyn Error>> {
        info!("statically processing");

        let mut buf = String::new();
        std::fs::File::open(path)?.read_to_string(&mut buf)?;

        Ok(buf)
    }
}

#[derive(Default, Debug, Clone)]
struct Store {
    hm: HashMap<PathBuf, String>,
}

impl Store {
    fn put<P: Into<PathBuf>>(self: &mut Self, path: P, content: String) {
        let path = path.into();
        debug!(?path, content_length = content.len(), "putting into store");
        self.hm.insert(path, content);
    }

    fn get<P: AsRef<Path>>(self: &Self, path: P) -> Option<&str> {
        self.hm.get(path.as_ref()).map(|s| s.as_str())
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

fn find_and_process() -> Result<Store, Box<dyn Error>> {
    let mut paths = Vec::new();
    let cwd = std::env::current_dir()?;

    walk(&cwd, &mut paths)?;

    let mut store = Store::default();

    for path in paths {
        if !StaticProcessor::matches(&path) {
            break;
        }

        let short_path = path.strip_prefix(&cwd)?;
        store.put(short_path, StaticProcessor::process(&path)?);
    }

    Ok(store)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let store = sync::Arc::new(sync::Mutex::new(find_and_process()?));

    let app = Router::new()
        .fallback(get(root)).layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(Extension(store)),
    );

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn root(store: Extension<sync::Arc<sync::Mutex<Store>>>) -> String {
    let handle = store.0.lock().unwrap();
    handle.get("test-site/index.html").unwrap().to_string()
}
