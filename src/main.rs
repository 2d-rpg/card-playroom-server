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

pub mod card;
pub mod codec;
pub mod deck;
pub mod edit_deck;
pub mod graphql;
pub mod index;
pub mod models;
pub mod schema;
pub mod upload;
pub mod ws_actors;
pub mod ws_session;

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;
pub type DbCon = r2d2::PooledConnection<ConnectionManager<PgConnection>>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Set logger
    env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    env_logger::init();

    let db_pool = create_db_pool();

    // Start game server actor
    let ws_server = ws_actors::ChatServer::default().start();

    // Start tcp server in separate thread
    let srv = ws_server.clone();
    ws_session::tcp_server("127.0.0.1:12345", srv);

    println!("Started http server: 127.0.0.1:8080");

    // Start http server
    HttpServer::new(move || {
        App::new()
            .data(db_pool.clone())
            .data(ws_server.clone())
            .wrap(Logger::default())
            .wrap(
                Cors::default()
                    .allowed_origin("127.0.0.1:12345")
                    .allow_any_origin() // TODO: デプロイ時にサーバのドメインを書けばいいのか調べる
                    .allowed_methods(vec!["POST", "GET"])
                    .allowed_headers(vec![
                        header::CONTENT_TYPE,
                        header::AUTHORIZATION,
                        header::ACCEPT,
                    ])
                    .supports_credentials()
                    .max_age(3600),
            )
            .configure(graphql::register)
            .configure(ws_session::register)
            .configure(index::register)
            .configure(card::register)
            .configure(edit_deck::register)
            .configure(deck::register)
            .configure(upload::register)
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
