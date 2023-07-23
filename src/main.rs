use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Router};
use lumin::processors::{LiquidProcessor, PostsProcessor, StaticProcessor};
use lumin::store::{find_and_process, Store};
use std::error::Error;
use std::net::SocketAddr;
use std::path::Path;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{debug, info};

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let path = if let Some(arg) = std::env::args().skip(1).take(1).next() {
        arg.into()
    } else {
        std::env::current_dir()?
    };

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

    let store = find_and_process(path, &[&p, &l, &s])?;

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
