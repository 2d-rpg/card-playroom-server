use crate::ws_actors;
use actix::prelude::*;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use std::time::{Duration, Instant};

use crate::ws_actors::WsActor;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// メインのアクターサーバー
struct WsSession {
    id: u32,
    hb: Instant,
    addr: Addr<WsActor>,
}

impl WsSession {
    /// ハートビート
    ///
    /// クライアントの接続/切断を一定時間ごとに確認するためのもの
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                println!("Websocket Client heartbeat failed, disconnecting!");
                act.addr.do_send(ws_actors::Disconnect { id: act.id });
                ctx.stop();
                return;
            }
            ctx.ping(b""); // `ping`のコール
        });
    }
}

/// `Actor`トレイトの実装
impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    /// クライアントの接続処理
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);

        let addr = ctx.address();
        self.addr
            .send(ws_actors::Connect {
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => act.id = res, // ユニークIDを付与
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    /// クライアントの切断処理
    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        self.addr.do_send(ws_actors::Disconnect { id: self.id });
        Running::Stop
    }
}

//
// クライアントからの要求を処理するハンドラーの実装
//

impl Handler<ws_actors::Message> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: ws_actors::Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

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
                    msg: m.to_string(),
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

/// ルーティング
async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<WsActor>>,
) -> Result<HttpResponse, Error> {
    ws::start(
        WsSession {
            id: 0,
            hb: Instant::now(),
            addr: srv.get_ref().clone(),
        },
        &req,
        stream,
    )
}

pub fn register(config: &mut web::ServiceConfig) {
    let ws_server = WsActor::new().start(); // WebSocketアクターサーバーを開始

    config
        .data(ws_server.clone())
        .service(web::resource("/ws/").to(ws_route));
}
