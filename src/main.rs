use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Router};
use lumin::processors::{LiquidProcessor, StaticProcessor};
use lumin::store::{find_and_process, Store};
use std::error::Error;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let path = if let Some(arg) = std::env::args().skip(1).take(1).next() {
        arg.into()
    } else {
        std::env::current_dir()?
    };

    let s = StaticProcessor {};
    let l = LiquidProcessor::new(path.join("partials"))?;

    let store = find_and_process(path, &[&s, &l])?;

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
    let path = request.uri().path().trim_start_matches('/');
    if let Some(res) = store.get(path) {
        return res.into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}
