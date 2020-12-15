use crate::ws_actors;
use actix::prelude::*;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use crate::ws_actors::WsActor;
use crate::{DbCon, DbPool};

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

struct WsSession {
    /// unique session id
    id: u32,
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    hb: Instant,
    /// joined room
    room: String,
    /// peer name
    name: Option<String>,
    /// Game server
    addr: Addr<WsActor>,
}

impl WsSession {
    /// helper method that sends ping to client every second.
    ///
    /// also this method checks heartbeats from client
    ///
    /// クライアントの接続/切断を一定時間ごとに確認するためのもの
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                println!("Websocket Client heartbeat failed, disconnecting!");
                // notify game server
                act.addr.do_send(ws_actors::Disconnect {
                    id: act.id,
                    room: act.room,
                });
                // stop actor
                ctx.stop();
                // don't try to send a ping
                return;
            }
            // send a ping
            ctx.ping(b"");
        });
    }
}

/// `Actor`トレイトの実装
impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    /// クライアントの接続処理
    fn started(&mut self, ctx: &mut Self::Context) {
        // we'll start heartbeat process on session start.
        self.hb(ctx);

        // register self in chat server. `AsyncContext::wait` register
        // future within context, but context waits until this future resolves
        // before processing any other events.
        // HttpContext::state() is instance of WsSessionState, state is shared
        // across all routes within application
        let addr = ctx.address();
        self.addr
            .send(ws_actors::Connect {
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    // uniq id is set to response
                    Ok(res) => act.id = res,
                    // something is wrong with chat server
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    /// クライアントの切断処理
    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        // notify chat server
        self.addr.do_send(ws_actors::Disconnect {
            id: self.id,
            room: self.room,
        });
        Running::Stop
    }
}

//
// クライアントからの要求を処理するハンドラーの実装
//
impl Handler<ws_actors::Message> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: ws_actors::Message, ctx: &mut Self::Context) {
        ctx.text(msg.0.to_string()); // WebsocketDataのto_stringの実装
    }
}

/// WebSocket message handler
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    /// クライアントとの送受信においてどのようなデータをどう処理するか
    ///
    /// `Text`であれば受信したメッセージをWebSocketアクターサーバーにそのクライアントのIDと共に渡す
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let msg = match msg {
            Err(_) => {
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };

        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => {
                let m = text.trim();

                self.addr.do_send(ws_actors::ClientMessage {
                    id: self.id,
                    data: ws_actors::WebsocketData::Message(m.to_string()),
                    room: "".to_string(), // TODO: room検索
                })
            }
            ws::Message::Binary(_) => println!("Unexpected binary"),
            ws::Message::Close(_) => {
                ctx.stop();
            }
            ws::Message::Continuation(_) => {
                ctx.stop();
            }
            ws::Message::Nop => (),
        }
    }
}

///  Displays and affects state
// async fn get_count(count: web::Data<Arc<AtomicUsize>>) -> impl Responder {
//     let current_count = count.fetch_add(1, Ordering::SeqCst);
//     format!("Visitors: {}", current_count)
// }

/// Entry point for our websocket route
async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<WsActor>>,
    // db_pool: web::Data<DbPool>,
) -> Result<HttpResponse, Error> {
    // print request headers
    for x in req.headers().iter() {
        println!("{:?}", x);
    }
    // start websocket
    ws::start(
        WsSession {
            id: 0,
            hb: Instant::now(),
            room: "".to_string(), // TODO: Optionにするか検討
            name: None,
            addr: srv.get_ref().clone(),
        },
        &req,
        stream,
    )
}

pub fn register(config: &mut web::ServiceConfig) {
    // App state
    // We are keeping a count of the number of visitors
    let app_state = Arc::new(AtomicUsize::new(0));

    // Start game server actor
    let ws_server = WsActor::new(app_state.clone()).start();

    config
        .data(ws_server.clone())
        .service(web::resource("/ws").to(ws_route));
}
