use axum::{extract::Extension, routing::get, Router, Server};
use std::net::SocketAddr;
use std::sync::Arc;
use ws_server::{list_handler, ws_handler, State};

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    let state = Arc::new(State::default());

    let app = Router::new()
        .route("/ws", get(ws_handler).layer(Extension(state.clone())))
        .route("/list", get(list_handler).layer(Extension(state.clone())));

    println!("Listening on {}", addr);
    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
