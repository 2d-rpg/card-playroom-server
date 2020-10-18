extern crate dotenv;
extern crate serde;
use actix_web::middleware::Logger;
use actix_web::{get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Serialize, Deserialize, Debug)]
struct RoomInfo {
    rooms: Vec<Room>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Room {
    name: String,
    id: String,
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

#[get("/rooms")]
async fn rooms(_req: HttpRequest) -> impl Responder {
    let room1 = Room {
        name: String::from("Room1"),
        id: String::from("shimosakakenhablackkenkyushitsudesu"),
    };
    let room2 = Room {
        name: String::from("ルーム2"),
        id: String::from("muratakenhahouninshugidesu"),
    };
    let room_info = RoomInfo {
        rooms: vec![room1, room2],
    };
    println!("{:?}", room_info);
    HttpResponse::Ok().json(room_info)
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let endpoint = dotenv::var("ENDPOINT").unwrap() + ":8080";
    env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();
    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .service(hello)
            .service(echo)
            .service(rooms)
            .route("/hey", web::get().to(manual_hello))
    })
    .bind(endpoint)?
    .run()
    .await
}
