use const_format::concatcp;
use dotenv::dotenv;
use dotenv_codegen::dotenv;
use poem::{endpoint::StaticFilesEndpoint, listener::TcpListener, EndpointExt, Route, Server};
use poem_grants::GrantsMiddleware;
use poem_openapi::OpenApiService;

use crate::{auth::AuthApi, extractor::auth_extractor};

pub mod auth;
pub mod extractor;

const PORT: u16 = 80;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    color_eyre::install().unwrap();
    // Logging setup
    let tracing_sub = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(tracing_sub).unwrap();
    dotenv().ok();

    // Init services
    let main_service = OpenApiService::new(AuthApi::new(), "Main", "1.0.0")
        .server(concatcp!("http://localhost:", PORT, "/api"));

    let app = Route::new()
        .nest(
            "/",
            StaticFilesEndpoint::new("public/")
                .show_files_listing()
                .index_file("index.html")
                .with(GrantsMiddleware::with_extractor(auth_extractor))
        )
        .nest(
            "/api",
            Route::new()
                .nest("/docs", main_service.swagger_ui())
                .nest("/", main_service)
                .with(GrantsMiddleware::with_extractor(auth_extractor))
        );

    let listener = TcpListener::bind(("0.0.0.0", PORT));
    println!(
        "Starting server on port {} (http://localhost:{})",
        PORT, PORT
    );
    Server::new(listener).run(app).await?;

    Ok(())
}

