//! Tourcalc's server, in Rust.
//!
//! Phase 2 of the port: read-only, so that it can be run beside the C# one and checked by
//! pointing the existing Blazor client at it. If that client cannot tell the difference,
//! the contract is right.

use axum::Router;
use std::sync::Arc;
use tc_server::{api, auth, config, state, store};
use tower_http::services::{ServeDir, ServeFile};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tc_server=info,tower_http=warn".into()),
        )
        .init();

    let cfg = config::Config::from_env();

    let signer = match auth::Signer_::from_base64(&cfg.private_key_b64) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("{e}");
            std::process::exit(1);
        }
    };

    let store = match store::InMemoryStore::from_seed_file(std::path::Path::new(&cfg.seed_file)) {
        Ok(s) => {
            tracing::info!("{} tours from {}", s.len(), cfg.seed_file);
            s
        }
        Err(e) => {
            tracing::error!("could not read the seed file: {e}");
            std::process::exit(1);
        }
    };

    let state: state::Shared = Arc::new(state::AppState {
        store: Box::new(store),
        signer,
        master_key: cfg.master_key.clone(),
        token_valid_minutes: cfg.token_valid_minutes,
        started: std::time::SystemTime::now(),
    });

    let mut app = Router::new().merge(api::routes(state));

    // Optionally serve the built Blazor client from here as well, which is what makes the
    // "point the old client at the new server" test possible without a proxy in between.
    if let Some(dir) = &cfg.static_dir {
        let index = std::path::Path::new(dir).join("index.html");
        app = app.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
        tracing::info!("serving {dir}");
    }

    let app = app.layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = match tokio::net::TcpListener::bind(&cfg.listen).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("cannot listen on {}: {e}", cfg.listen);
            std::process::exit(1);
        }
    };
    tracing::info!("listening on http://{}", cfg.listen);

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("server");
}
