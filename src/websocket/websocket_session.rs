use std::time::Instant;

use actix::Addr;
use actix_web_actors::ws;
use uuid::Uuid;

use super::Message;
use super::*;

/// `WsChatSession` is Actor for websocket
pub struct WsChatSession {
    // TODO newの実装を行い、pubをつけないように変更
    /// unique session id
    pub id: Uuid,
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    pub hb: Instant,
    /// joined room
    pub room: Option<Uuid>,
    /// peer name
    pub name: Option<String>,
    /// Chat server
    pub addr: Addr<room_manager::ChatServer>,
}

impl Actor for WsChatSession {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start.
    /// We register ws session with ChatServer
    fn started(&mut self, ctx: &mut Self::Context) {
        // we'll start heartbeat process on session start.
        self.hb(ctx);

        // register self in chat server. `AsyncContext::wait` register
        // future within context, but context waits until this future resolves
        // before processing any other events.
        // HttpContext::state() is instance of WsChatSessionState, state is shared
        // across all routes within application
        let addr = ctx.address();
        self.addr
            .send(Connect {
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => act.id = res,
                    // something is wrong with chat server
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        // notify chat server
        self.addr.do_send(Disconnect { id: self.id });
        Running::Stop
    }
}

/// Handle messages from chat server, we simply send it to peer websocket
impl Handler<ChatMessage> for WsChatSession {
    type Result = ();

    fn handle(&mut self, msg: ChatMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

/// WebSocket message handler
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsChatSession {
    // TODO: "/command"形式ではなく、json形式に揃える
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let msg = match msg {
            Err(error) => {
                println!("{}", error);
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };

        println!("WEBSOCKET MESSAGE: {:?}", msg);
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
                // we check for /sss type of messages
                if m.starts_with('/') {
                    let v: Vec<&str> = m.splitn(2, ' ').collect();
                    match v[0] {
                        "/list" => {
                            // Send ListRooms message to chat server and wait for
                            // response
                            println!("List rooms");
                            self.addr
                                .send(ListRooms)
                                .into_actor(self)
                                .then(|res, _, ctx| {
                                    match res {
                                        Ok(rooms) => {
                                            ctx.text(
                                                rooms.get_json_data(Status::Ok, Event::GetRoomList),
                                            );
                                        }
                                        _ => println!("Something is wrong"),
                                    }
                                    fut::ready(())
                                })
                                .wait(ctx)
                            // .wait(ctx) pauses all events in context,
                            // so actor wont receive any new messages until it get list
                            // of rooms back
                        }
                        "/join" => {
                            if v.len() == 2 {
                                self.room = Some(Uuid::parse_str(v[1]).unwrap());
                                self.addr
                                    .send(Join {
                                        session_id: self.id,
                                        room_id: Uuid::parse_str(v[1]).unwrap(),
                                    })
                                    .into_actor(self)
                                    .then(|res, _, ctx| {
                                        match res {
                                            Ok(room_info) => {
                                                ctx.text(
                                                    room_info.get_json_data(
                                                        Status::Ok,
                                                        Event::EnterRoom,
                                                    ),
                                                );
                                            }
                                            _ => println!("Something is wrong!"),
                                        }
                                        fut::ready(())
                                    })
                                    .wait(ctx)
                            } else {
                                ctx.text(
                                    SimpleMessage {
                                        message: "!!! room id is required".to_string(),
                                    }
                                    .get_json_data(Status::Error, Event::EnterRoom),
                                );
                            }
                        }
                        "/create" => {
                            if v.len() == 2 {
                                self.addr
                                    .send(Create {
                                        session_id: self.id,
                                        room_name: v[1].to_owned(),
                                    })
                                    .into_actor(self)
                                    .then(|res, _, ctx| {
                                        match res {
                                            Ok(createroom) => {
                                                let data = RoomInfo {
                                                    id: createroom.room_id,
                                                    name: createroom.room_name,
                                                    num: 0,
                                                };
                                                ctx.text(
                                                    data.get_json_data(
                                                        Status::Ok,
                                                        Event::CreateRoom,
                                                    ),
                                                );
                                            }
                                            // TODO: statusをerrorとして返した方がよい？
                                            _ => println!("Something is wrong"),
                                        }
                                        fut::ready(())
                                    })
                                    .wait(ctx)
                            } else {
                                ctx.text(
                                    SimpleMessage {
                                        message: "!!! room name is required".to_string(),
                                    }
                                    .get_json_data(Status::Error, Event::Unknown),
                                );
                            }
                        }
                        "/cards" => {
                            if v.len() == 2 {
                                let msg = v[1].to_owned();
                                dbg!(&msg);
                                let card_info: Vec<CardInfo> = serde_json::from_str(&msg).unwrap();
                                let card_info_list = CardInfoList { cards: card_info };
                                if let Some(room) = self.room {
                                    self.addr.do_send(Message {
                                        id: self.id,
                                        msg: card_info_list
                                            .get_json_data(Status::Ok, Event::CardsInfo),
                                        room: room,
                                    })
                                }
                            } else {
                                ctx.text(
                                    SimpleMessage {
                                        message: "!!! cards info is required".to_string(),
                                    }
                                    .get_json_data(Status::Error, Event::Unknown),
                                );
                            }
                        }
                        _ => ctx.text(
                            SimpleMessage {
                                message: format!("!!! unknown command: {:?}", m),
                            }
                            .get_json_data(Status::Error, Event::Unknown),
                        ),
                    }
                } else {
                    ctx.text(
                        SimpleMessage {
                            message: "!!! message must starts with /".to_string(),
                        }
                        .get_json_data(Status::Error, Event::Unknown),
                    );
                }
            }
            ws::Message::Binary(_) => println!("Unexpected binary"),
            ws::Message::Close(reason) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => (),
        }
    }
}

impl WsChatSession {
    /// helper method that sends ping to client every second.
    ///
    /// also this method checks heartbeats from client
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                println!("Websocket Client heartbeat failed, disconnecting!");

                // notify chat server
                act.addr.do_send(Disconnect { id: act.id });

                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }

            ctx.ping(b"");
        });
    }
}
