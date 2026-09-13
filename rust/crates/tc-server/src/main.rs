//! Tourcalc's server, in Rust.
//!
//! Phase 2 of the port: read-only, so that it can be run beside the C# one and checked by
//! pointing the existing Blazor client at it. If that client cannot tell the difference,
//! the contract is right.

use axum::response::IntoResponse;
use axum::Router;
use std::sync::Arc;
use tc_server::{api, auth, config, state, store};
use tower_http::services::{ServeDir, ServeFile};

/// The wasm file index.html names, hash and all - the one identifier of a client build that
/// cannot drift, because the hash is of the contents.
fn client_asset(static_dir: Option<&str>) -> Option<String> {
    let page = std::fs::read_to_string(std::path::Path::new(static_dir?).join("index.html")).ok()?;
    Some(tc_server::wasm_named_in(&page)?.to_owned())
}

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

    // Which store, by the same setting the C# reads.
    // Filled in with the tour store when that store is a database, so that subscriptions
    // live where the tours do. Without it they sit in memory and every new image - which is
    // to say every deploy - quietly unsubscribes everybody who asked to be told.
    let mut subscriptions: Option<Box<dyn tc_server::subscriptions::SubscriptionStore>> = None;

    let store: Box<dyn store::TourStore> = if cfg.storage_type.eq_ignore_ascii_case("MongoDb") {
        #[cfg(feature = "mongo")]
        {
            match tc_server::mongo::MongoStore::connect(
                &cfg.mongo_url,
                &cfg.mongo_username,
                &cfg.mongo_password,
            )
            .await
            {
                Ok(store) => {
                    tracing::info!("storing tours in MongoDB");
                    subscriptions = Some(Box::new(store.subscriptions()));
                    Box::new(store)
                }
                Err(e) => {
                    tracing::error!("{e}");
                    std::process::exit(1);
                }
            }
        }
        // Refusing plainly beats starting up and quietly keeping everything in memory: a
        // deployment that asked for a database and got a server that forgets everything on
        // restart would find out at the worst moment.
        #[cfg(not(feature = "mongo"))]
        {
            tracing::error!(
                "StorageType is MongoDb, but this build has no database: rebuild with \
                 --features mongo"
            );
            std::process::exit(1);
        }
    } else {
        match store::InMemoryStore::from_seed_file(std::path::Path::new(&cfg.seed_file)) {
            Ok(s) => {
                tracing::info!("{} tours from {}", s.len(), cfg.seed_file);
                Box::new(s)
            }
            Err(e) => {
                tracing::error!("could not read the seed file: {e}");
                std::process::exit(1);
            }
        }
    };

    // Notifications: the real thing when there are keys to sign with, silence otherwise -
    // which is what the C# does when the settings are missing.
    let push: Box<dyn tc_server::push::Notifier> = match tc_server::push::WebPush::new(
        &cfg.push_public_key,
        &cfg.push_private_key,
        &cfg.push_contact,
    ) {
        Some(real) => {
            tracing::info!("push notifications are configured");
            Box::new(real)
        }
        None => Box::new(tc_server::push::Silent),
    };

    let state: state::Shared = Arc::new(state::AppState {
        subscriptions: subscriptions.unwrap_or_else(|| {
            // In-memory tours, in-memory subscriptions: a server that forgets the trips
            // when it stops has nothing to notify anybody about afterwards.
            Box::new(tc_server::subscriptions::InMemorySubscriptions::default())
        }),
        push,
        store,
        signer,
        master_key: cfg.master_key.clone(),
        token_valid_minutes: cfg.token_valid_minutes,
        started: std::time::SystemTime::now(),
        versioning: cfg.versioning,
        version_editable: cfg.version_editable,
        max_tours_per_code: cfg.max_tours_per_code,
        wakeup_code: cfg.wakeup_code.clone(),
        wakeup_pre_delay_min: cfg.wakeup_pre_delay_min,
        wakeup_post_delay_min: cfg.wakeup_post_delay_min,
        build_type: cfg.build_type.clone(),
        build_id: cfg.build_id.clone(),
        build_commit: cfg.build_commit.clone(),
        // Which client this server hands out, read from the page it hands out. Once, at
        // startup: the files under a running container do not change, and a browser asking
        // "am I current?" should not cost a disk read.
        client_asset: client_asset(cfg.static_dir.as_deref()),
        wakeups: Default::default(),
    });

    let mut app = Router::new()
        .merge(api::routes(state.clone()))
        // The text interface, for browsers that cannot run the app at all.
        .merge(tc_server::text::routes().with_state(state))
        // Before the static files, so it wins over anything left in the directory with that
        // name: the Blazor client's service worker, dismissed.
        .route(
            "/service-worker.js",
            axum::routing::get(tc_server::retire_the_old_service_worker),
        );

    // Optionally serve the built Blazor client from here as well, which is what makes the
    // "point the old client at the new server" test possible without a proxy in between.
    if let Some(dir) = &cfg.static_dir {
        let index = std::path::Path::new(dir).join("index.html");
        app = app.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
        tracing::info!("serving {dir}");
    }

    // A text browser asking for the app gets the text pages instead.
    let agents = tc_server::text::redirect::parse_agents(&cfg.text_browser_agents);
    let redirecting = cfg.text_browser_redirect && !agents.is_empty();
    let text_browsers = axum::middleware::from_fn(
        move |request: axum::extract::Request, next: axum::middleware::Next| {
            let agents = agents.clone();
            async move {
                if redirecting {
                    let ua = request
                        .headers()
                        .get(axum::http::header::USER_AGENT)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    if let Some(target) = tc_server::text::redirect::target_for(
                        request.method().as_str(),
                        request.uri().path(),
                        ua,
                        &agents,
                    ) {
                        // Temporary: whether a reader wants the text pages is not something
                        // to write into their cache for good.
                        return axum::response::Redirect::temporary(&target).into_response();
                    }
                }
                next.run(request).await
            }
        },
    );

    // What the browser may keep, and for how long.
    //
    // "no-cache" belongs on what can differ between two requests: the app shell, the API.
    // It does *not* belong on the hashed assets - `tc-web-<hash>.wasm` is named after its
    // own contents, so it can be kept until its name changes - and putting it there was
    // what made the C# ask about every framework file on every load, a round trip each,
    // for answers that were always 304.
    let cache_headers = axum::middleware::from_fn(
        |request: axum::extract::Request, next: axum::middleware::Next| async move {
            let path = request.uri().path().to_owned();
            let is_api = path.starts_with("/api");
            let mut response = next.run(request).await;
            let is_document = response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.starts_with("text/html"));
            // Every answer says how long it may be kept. Saying nothing is not neutral: a
            // browser then guesses from the file's age, and the guess is what left one
            // client's manifest in front of another client's app.
            response.headers_mut().insert(
                axum::http::header::CACHE_CONTROL,
                axum::http::HeaderValue::from_static(tc_server::cache_for(
                    &path,
                    is_api,
                    is_document,
                )),
            );
            response
        },
    );

    // Compress what goes out. The features were enabled from the start and the layer was
    // never added, so the client was downloading a 690 K wasm file that brotli takes to
    // about 225 K - on a project whose whole premise is the size of that file. Negotiated
    // per request: a client that asks for neither gets the bytes as they are.
    // Which build answered, visible in the network tab without asking anybody.
    let version = axum::http::HeaderValue::from_str(&format!("rust v {}", cfg.build_type))
        .unwrap_or_else(|_| axum::http::HeaderValue::from_static("rust"));

    let app = app
        .layer(text_browsers)
        .layer(cache_headers)
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::HeaderName::from_static("x-tourcalc-version"),
            version,
        ))
        // Anywhere, any method, any header, credentials included - the C#'s policy. It is
        // as open as a policy gets, and it is not what protects anything here: a tour is
        // reached with a token, and a token comes from an access code.
        //
        // Everything is *mirrored* rather than answered with `*`, and that is not a
        // preference. A wildcard alongside `Allow-Credentials: true` is invalid CORS and
        // browsers reject it; tower-http refuses to build such a layer at all, which is how
        // this was found - the server would not start. ASP.NET quietly echoes the request in
        // the same situation, so mirroring is also what the C# actually sends.
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::AllowOrigin::mirror_request())
                .allow_methods(tower_http::cors::AllowMethods::mirror_request())
                .allow_headers(tower_http::cors::AllowHeaders::mirror_request())
                .allow_credentials(true),
        )
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = match tokio::net::TcpListener::bind(&cfg.listen).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("cannot listen on {}: {e}", cfg.listen);
            std::process::exit(1);
        }
    };
    tracing::info!("listening on http://{}", cfg.listen);

    axum::serve(listener, app)
        .with_graceful_shutdown(stop_asked())
        .await
        .expect("server");
}

/// Waits for somebody to ask the server to stop.
///
/// Ctrl-C is how it is stopped at a desk. **SIGTERM is how a container is stopped**, and
/// that is the one that matters in a deployment: `podman stop` sends it, waits ten seconds,
/// and then kills. With no handler for it the process simply dies on the signal - the
/// default disposition - and whatever request was in flight dies with it. Waiting for the
/// answers already being written costs nothing and is the difference between a deploy
/// nobody notices and one somebody's save falls into.
async fn stop_asked() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            // Nothing to be done about it, and pretending to wait forever is better than
            // shutting down because the handler could not be installed.
            Err(e) => {
                tracing::error!("cannot listen for SIGTERM: {e}");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = interrupt => tracing::info!("interrupted, stopping"),
        _ = terminate => tracing::info!("asked to stop"),
    }
}
