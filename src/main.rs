mod api;
mod database;
mod redis_db;
mod rpc;

use dotenv::dotenv;
use std::env;

use actix_cors::Cors;
use actix_web::http::header;
use actix_web::{get, middleware, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
pub struct AppState {
    pub db: clickhouse::Client,
    pub redis_client: redis::Client,
}

async fn greet() -> impl Responder {
    HttpResponse::Ok().body("Hello, Actix Web!")
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        // .with_env_filter(EnvFilter::new("debug"))
        .with_writer(std::io::stderr)
        .init();

    let db = database::establish_connection();
    let redis_client =
        redis::Client::open(env::var("REDIS_URL").expect("Missing REDIS_URL env var"))
            .expect("Failed to connect to Redis");

    HttpServer::new(move || {
        // Configure CORS middleware
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec![
                header::CONTENT_TYPE,
                header::AUTHORIZATION,
                header::ACCEPT,
            ])
            .max_age(3600)
            .supports_credentials();

        let mut apis = web::scope("/v0")
            .service(api::lookup_by_public_key)
            .service(api::lookup_by_public_key_all)
            .service(api::staking)
            .service(api::ft)
            .service(api::nft);
        if env::var("ENABLE_EXPERIMENTAL").ok() == Some("true".to_string()) {
            apis = apis
                .service(api::account_keys)
                .service(api::ft_with_balances);
        }

        App::new()
            .app_data(web::Data::new(AppState {
                db: db.clone(),
                redis_client: redis_client.clone(),
            }))
            .wrap(cors)
            .wrap(middleware::Logger::new(
                "%{r}a \"%r\"	%s %b \"%{Referer}i\" \"%{User-Agent}i\" %T",
            ))
            .wrap(tracing_actix_web::TracingLogger::default())
            .service(apis)
            .route("/", web::get().to(greet))
    })
    .bind(format!("127.0.0.1:{}", env::var("PORT").unwrap()))?
    .run()
    .await?;

    Ok(())
}
