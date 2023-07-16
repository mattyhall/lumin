mod store;

use axum::http::Request;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Router};
use std::error::Error;
use std::net::SocketAddr;
use std::sync;
use store::{find_and_process, Store};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let cwd = std::env::current_dir()?;
    let store = sync::Arc::new(find_and_process(cwd)?);

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

async fn root<T>(store: Extension<sync::Arc<Store>>, request: Request<T>) -> impl IntoResponse {
    let path = request.uri().path().trim_start_matches('/');
    store.get(path).unwrap().to_owned()
}
