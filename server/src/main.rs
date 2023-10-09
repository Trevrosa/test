use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Html;
use axum::{response::IntoResponse, routing::get, Router};

use clap::Parser;

use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;

use tokio::fs;
use tower::{ServiceBuilder, ServiceExt};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

// Setup the command line interface with clap.
#[derive(Parser, Debug)]
#[clap(name = "server", about = "A server for our wasm project!")]
struct Opt {
    /// set the log level
    #[clap(short = 'l', long = "log", default_value = "debug")]
    log_level: String,

    /// set the listen addr
    #[clap(short = 'a', long = "addr", default_value = "::1")]
    addr: String,

    /// set the listen port
    #[clap(short = 'p', long = "port", default_value = "8080")]
    port: u16,

    /// set the directory where static files are to be found
    #[clap(long = "static-dir", default_value = "../dist")]
    static_dir: String,
}

#[tokio::main]
async fn main() {
    let opt: Opt = Opt::parse();

    // Setup logging & RUST_LOG from args
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", format!("{},hyper=info,mio=info", opt.log_level));
    }
    // enable console logging
    tracing_subscriber::fmt::init();

    let app: Router = Router::new()
        .route("/api/hello", get(hello))
        .fallback_service(get(|req: Request<Body>| async move {
            const ROUTES: &[&str] = &["/", "/hello-server"];
            let is_known_path = ROUTES.contains(&req.uri().path());
            let reqd = &req.uri().clone();

            let res = ServeDir::new(&opt.static_dir).oneshot(req).await.unwrap(); // serve dir is infallible
            let status = res.status();
            match (status, is_known_path) {
                // if we don't find a file corresponding to the path but the
                // path is a "known route", we serve index.html
                (StatusCode::NOT_FOUND, true) => {
                    let index_path: PathBuf = PathBuf::from(&opt.static_dir).join("index.html");
                    fs::read_to_string(index_path)
                        .await
                        .map(|index_content| (StatusCode::OK, Html(index_content)).into_response())
                        .unwrap_or_else(|_| {
                            (StatusCode::INTERNAL_SERVER_ERROR, "index.html not found")
                                .into_response()
                        })
                },
                // otherwise we serve a 404
                (StatusCode::NOT_FOUND, false) => {
                    (StatusCode::NOT_FOUND, format!("{reqd} not found")).into_response()
                }

                // path was found as a file in the static dir
                _ => res.into_response(),
            }
        }))
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    let sock_addr: SocketAddr = SocketAddr::from((
        IpAddr::from_str(opt.addr.as_str()).unwrap_or(IpAddr::V6(Ipv6Addr::LOCALHOST)),
        opt.port,
    ));

    tracing::info!("listening on http://{}", sock_addr);

    axum::Server::bind(&sock_addr)
        .serve(app.into_make_service())
        .await
        .expect("Unable to start server");
}

async fn hello() -> impl IntoResponse {
    "hello from server!"
}