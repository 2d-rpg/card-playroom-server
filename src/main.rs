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

mod schema;

use crate::schema::{create_schema, Schema};

/// How often heartbeat pings are sent
// const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
// const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// do websocket handshake and start `MyWebSocket` actor
// async fn ws_index(r: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
//     println!("{:?}", r);
//     let res = ws::start(MyWebSocket::new(), &r, stream);
//     println!("{:?}", res);
//     res
// }

/// websocket connection is long running connection, it easier
/// to handle with an actor
// struct MyWebSocket {
/// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
/// otherwise we drop connection.
//     hb: Instant,
// }

// impl Actor for MyWebSocket {
//     type Context = ws::WebsocketContext<Self>;

//     /// Method is called on actor start. We start the heartbeat process here.
//     fn started(&mut self, ctx: &mut Self::Context) {
//         self.hb(ctx);
//     }
// }

/// Handler for `ws::Message`
// impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWebSocket {
//     fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
//         // process websocket messages
//         println!("WS: {:?}", msg);
//         match msg {
//             Ok(ws::Message::Ping(msg)) => {
//                 self.hb = Instant::now();
//                 ctx.pong(&msg);
//             }
//             Ok(ws::Message::Pong(_)) => {
//                 self.hb = Instant::now();
//             }
//             Ok(ws::Message::Text(text)) => ctx.text(text),
//             Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
//             Ok(ws::Message::Close(reason)) => {
//                 ctx.close(reason);
//                 ctx.stop();
//             }
//             _ => ctx.stop(),
//         }
//     }
// }

// impl MyWebSocket {
//     fn new() -> Self {
//         Self { hb: Instant::now() }
//     }

/// helper method that sends ping to client every second.
///
/// also this method checks heartbeats from client
// fn hb(&self, ctx: &mut <Self as Actor>::Context) {
//     ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
// check client heartbeats
// if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
// heartbeat timed out
// println!("Websocket Client heartbeat failed, disconnecting!");

// stop actor
// ctx.stop();

// don't try to send a ping
// return;
// }

//             ctx.ping(b"");
//         });
//     }
// }

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

// #[get("/rooms")]
// async fn rooms(_req: HttpRequest) -> impl Responder {
//     let room1 = Room {
//         name: String::from("Room1"),
//         id: String::from("shimosakakenhablackkenkyushitsudesu"),
//     };
//     let room2 = Room {
//         name: String::from("ルーム2"),
//         id: String::from("muratakenhahouninshugidesu"),
//     };
//     let room_info = RoomInfo {
//         rooms: vec![room1, room2],
//     };
//     println!("{:?}", room_info);
//     HttpResponse::Ok().json(room_info)
// }

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

/// GET用
///
/// 現在はhtmlを返すだけ(htmlファイルを指定していないので，実質何も返さない)
async fn graphiql() -> HttpResponse {
    // Set ENDPOINT
    let mut endpoint_url = "http://".to_owned();
    let endpoint = dotenv::var("ENDPOINT").unwrap() + ":8080/graphql";
    endpoint_url.push_str(&endpoint);
    let html = graphiql_source(&endpoint_url);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// POST用
async fn graphql(
    st: web::Data<Arc<Schema>>,
    data: web::Json<GraphQLRequest>,
) -> Result<HttpResponse, Error> {
    let user = web::block(move || {
        let res = data.execute(&st, &());
        Ok::<_, serde_json::error::Error>(serde_json::to_string(&res)?)
    })
    .await?;
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(user))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Set ENDPOINT from env var
    let endpoint = dotenv::var("ENDPOINT").expect("ENDPOINT must be set in .env file!") + ":8080";
    // Set logger
    env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    env_logger::init();
    // Create Juniper schema
    let schema = std::sync::Arc::new(create_schema());

    // set up database connection pool
    // let connspec = dotenv::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file!");
    // let manager = ConnectionManager::<SqliteConnection>::new(connspec);
    // let pool = r2d2::Pool::builder()
    //     .build(manager)
    //     .expect("Failed to create pool.");

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
            .service(web::resource("/graphql").route(web::post().to(graphql))) // POST -> GraphQL
            .service(web::resource("/graphiql").route(web::get().to(graphiql))) // GET -> GraphGL
            .route("/hey", web::get().to(manual_hello))
    })
    .bind(endpoint)?
    .run()
    .await
}
