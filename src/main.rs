#[macro_use]
extern crate diesel;
extern crate dotenv;

use std::env;
// use std::time::{Duration, Instant};

// use actix::prelude::*;
use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{http::header, App, HttpServer};
// use actix_web_actors::ws;

use diesel::{
    prelude::*,
    r2d2::{self, ConnectionManager},
};

pub mod admin;
pub mod graphql;
pub mod models;
pub mod schema;

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;
pub type DbCon = r2d2::PooledConnection<ConnectionManager<PgConnection>>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Set logger
    env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    env_logger::init();

    let db_pool = create_db_pool();

    // Start http server
    HttpServer::new(move || {
        App::new()
            .data(db_pool.clone())
            .wrap(Logger::default())
            .wrap(
                Cors::default()
                    .allowed_origin("http://localhost:8080") // TODO デプロイ時のドメインに対応
                    .allowed_origin("http://127.0.0.1:8080")
                    .allowed_methods(vec!["POST", "GET"])
                    .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
                    .allowed_header(header::CONTENT_TYPE)
                    .supports_credentials()
                    .max_age(3600),
            )
            // .service(web::resource("/ws/").route(web::get().to(ws_index)))
            .configure(graphql::register)
            .configure(admin::register)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}

fn create_db_pool() -> DbPool {
    let database_url = dotenv::var("DATABASE_URL").expect("DATABASE_URL must be set");

    r2d2::Pool::builder()
        .max_size(3)
        .build(ConnectionManager::<PgConnection>::new(database_url))
        .expect("failed to create db connection pool")
}
