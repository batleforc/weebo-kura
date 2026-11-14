//! The server binds both ports, answers, and stops when the token is cancelled.

#![expect(clippy::unwrap_used, reason = "test helpers fail the test on purpose")]

mod common;

use std::sync::Arc;

use kura_server::app::AppState;
use kura_server::metrics::Metrics;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

async fn http_get(addr: std::net::SocketAddr, path: &str) -> String {
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.write_all(format!("GET {path} HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n").as_bytes())
        .await
        .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    out
}

#[tokio::test]
async fn serves_api_and_metrics_then_drains_on_shutdown() {
    let api = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let metrics = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (api_addr, metrics_addr) = (api.local_addr().unwrap(), metrics.local_addr().unwrap());
    let state = Arc::new(AppState::new(
        Metrics::new().unwrap(),
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://nobody@127.0.0.1:1/none")
            .unwrap(),
        common::no_key_verifier(),
    ));
    let token = CancellationToken::new();
    let server = tokio::spawn(kura_server::serve(
        api,
        metrics,
        state,
        Vec::new(),
        token.clone(),
    ));

    assert!(
        http_get(api_addr, "/healthz")
            .await
            .starts_with("HTTP/1.1 200")
    );
    let m = http_get(metrics_addr, "/metrics").await;
    assert!(m.contains("kura_ready 0"), "{m}");
    assert!(
        http_get(api_addr, "/metrics")
            .await
            .starts_with("HTTP/1.1 404"),
        "metrics are not on the API port"
    );

    token.cancel();
    server.await.unwrap().unwrap();
}
