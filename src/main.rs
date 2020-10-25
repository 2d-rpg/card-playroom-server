#[macro_use]
extern crate diesel;
extern crate dotenv;

use std::env;
use std::sync::Arc;
// use std::time::{Duration, Instant};

use actix::prelude::*;
use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{get, post, web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
// use actix_web_actors::ws;
use juniper::http::graphiql::graphiql_source;
use juniper::http::GraphQLRequest;

use diesel::{
    prelude::*,
    r2d2::{self, ConnectionManager},
};

pub mod graphql;
pub mod models;
pub mod schema;

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;
pub type DbCon = r2d2::PooledConnection<ConnectionManager<PgConnection>>;

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Set ENDPOINT from env var
    let endpoint = dotenv::var("ENDPOINT").expect("ENDPOINT must be set in .env file!") + ":8080";
    // Set logger
    env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    env_logger::init();

    let db_pool = create_db_pool();

    // Start http server
    HttpServer::new(move || {
        App::new()
            .data(schema.clone())
            .wrap(Logger::default())
            .wrap(
                Cors::default()
                    .allowed_methods(vec!["POST", "GET"])
                    .supports_credentials()
                    .max_age(3600),
            )
            .service(hello)
            .service(echo)
            // .service(rooms)
            // .service(web::resource("/ws/").route(web::get().to(ws_index)))
            .configure(graphql::register)
            .route("/hey", web::get().to(manual_hello))
    })
    .bind(endpoint)?
    .run()
    .await
}

fn create_db_pool() -> DbPool {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    r2d2::Pool::builder()
        .max_size(3)
        .build(ConnectionManager::<PgConnection>::new(database_url))
        .expect("failed to create db connection pool")
}
