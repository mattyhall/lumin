use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Router};
use clap::Parser;
use lumin::ResourceProcessor;
use lumin::processors::{LiquidProcessor, PostsProcessor, StaticProcessor};
use lumin::store::{find_and_process, Store};
use notify_debouncer_full::notify::Watcher;
use std::error::Error;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{debug, info, error, instrument};

#[derive(Debug, Parser)]
struct Args {
    #[arg(help = "The site to serve")]
    site_path: PathBuf,
}

fn create_parser(partials_dir: impl AsRef<Path>) -> Result<liquid::Parser, Box<dyn Error>> {
    let mut ims = liquid::partials::InMemorySource::new();

    for entry in std::fs::read_dir(partials_dir.as_ref())? {
        let entry = entry?;
        if entry.metadata()?.is_dir() {
            continue;
        }

        let path = entry.path();
        debug!(?path, "found partial");

        let short_path = path.strip_prefix(partials_dir.as_ref())?;
        let buf = std::fs::read_to_string(&path)?;
        ims.add(short_path.file_name().unwrap().to_string_lossy(), buf);
    }

    let partials = liquid::partials::EagerCompiler::new(ims);

    Ok(liquid::ParserBuilder::new()
        .stdlib()
        .partials(partials)
        .build()?)
}

#[instrument(skip(store))]
fn rebuild(path: &Path, processors: &[&dyn ResourceProcessor], store: Store) -> Result<(), Box<dyn Error>> {
    let new_store = find_and_process(path, processors)?;
    store.replace(new_store);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let path = args.site_path;

    let partials_dir = path.join("partials");
    let parser = create_parser(&partials_dir)?;

    let s = StaticProcessor {};
    let p = PostsProcessor::new(
        path.join("posts"),
        path.join("post.liquid"),
        path.join("post_list.liquid"),
        &parser,
    )?;
    let l = LiquidProcessor::new(partials_dir, parser);
    let processors: &[&dyn ResourceProcessor] = &[&p, &l, &s];
    let store = find_and_process(&path, processors)?;

    let new_store = store.clone();
    let new_path = path.clone();

    let mut debouncer = notify_debouncer_full::new_debouncer(Duration::from_millis(250), None, move |res: notify_debouncer_full::DebounceEventResult| {
        let processors: &[&dyn ResourceProcessor] = &[&p, &l, &s];
        let path = new_path.clone();
        let store = new_store.clone();
        info!("files changed");
        match res {
            Ok(events) => events.into_iter().for_each(|ev| debug!(?ev, "got notify event")),
            Err(errors) => errors.into_iter().for_each(|e| error!(?e, "notify error")),
        }

        rebuild(&path, processors, store.clone()).expect("rebuild did not work");
    })?;
    debouncer.watcher().watch(&path, notify_debouncer_full::notify::RecursiveMode::Recursive)?;

    let app = Router::new().fallback(get(root)).layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(Extension(store)),
    );

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn root<T>(store: Extension<Store>, request: Request<T>) -> impl IntoResponse {
    let path = request.uri().path();
    if path == "/" {
        return store.get("index.html").unwrap().into_response();
    }

    let path = path.trim_start_matches('/');
    if let Some(res) = store.get(path) {
        return res.into_response();
    }

    let path = path.trim_end_matches('/');
    if let Some(res) = store.get(&format!("{}/index.html", path)) {
        return res.into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}
