//! `ClientSession` is an actor, it manages peer tcp connection and
//! proxies commands from peer to `ChatServer`.
use std::str::FromStr;
use std::time::{Duration, Instant};
use std::{io, net};

use futures::StreamExt;
use tokio::io::{split, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::FramedRead;

use actix::prelude::*;
// use actix_files as fs;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;

use crate::codec::{ChatCodec, ChatRequest, ChatResponse};
use crate::ws_actors::{self, ChatServer};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// Chat server sends this messages to session
#[derive(Message)]
#[rtype(result = "()")]
pub struct Message(pub String);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoomInfo {
    pub id: Uuid,
    pub name: String,
    pub num: usize,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoomInfoList {
    pub rooms: Vec<RoomInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorMessage {
    pub message: String,
}

pub trait MessageData {}

impl MessageData for RoomInfo {}
impl MessageData for RoomInfoList {}
impl MessageData for ErrorMessage {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsMessage<T: MessageData> {
    data: T,
    event: ws_actors::Event,
    status: ws_actors::Status,
}

impl RoomInfo {
    pub fn get_json_data(&self, status: ws_actors::Status, event: ws_actors::Event) -> String {
        serde_json::to_string(&WsMessage {
            data: self.clone(),
            event,
            status,
        })
        .unwrap()
    }
}

impl RoomInfoList {
    pub fn get_json_data(&self, status: ws_actors::Status, event: ws_actors::Event) -> String {
        serde_json::to_string(&WsMessage {
            data: self.clone(),
            event,
            status,
        })
        .unwrap()
    }
}

impl ErrorMessage {
    pub fn get_json_data(&self, status: ws_actors::Status, event: ws_actors::Event) -> String {
        serde_json::to_string(&WsMessage {
            data: self.clone(),
            event,
            status,
        })
        .unwrap()
    }
}

/// `ChatSession` actor is responsible for tcp peer communications.
pub struct ChatSession {
    /// unique session id
    id: Uuid,
    /// this is address of chat server
    addr: Addr<ChatServer>,
    /// Client must send ping at least once per 10 seconds, otherwise we drop
    /// connection.
    hb: Instant,
    /// joined room
    room: Option<Uuid>,
    /// Framed wrapper
    framed: actix::io::FramedWrite<ChatResponse, WriteHalf<TcpStream>, ChatCodec>,
}

impl Actor for ChatSession {
    /// For tcp communication we are going to use `FramedContext`.
    /// It is convenient wrapper around `Framed` object from `tokio_io`
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // we'll start heartbeat process on session start.
        self.hb(ctx);

        // register self in chat server. `AsyncContext::wait` register
        // future within context, but context waits until this future resolves
        // before processing any other events.
        let addr = ctx.address();
        self.addr
            .send(ws_actors::Connect {
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => act.id = res,
                    // something is wrong with chat server
                    _ => ctx.stop(),
                }
                actix::fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        // notify chat server
        self.addr.do_send(ws_actors::Disconnect { id: self.id });
        Running::Stop
    }
}

impl actix::io::WriteHandler<io::Error> for ChatSession {}

/// To use `Framed` we have to define Io type and Codec
impl StreamHandler<Result<ChatRequest, io::Error>> for ChatSession {
    /// This is main event loop for client requests
    fn handle(&mut self, msg: Result<ChatRequest, io::Error>, ctx: &mut Context<Self>) {
        match msg {
            Ok(ChatRequest::List) => {
                // Send ListRooms message to chat server and wait for response
                println!("List rooms");
                self.addr
                    .send(ws_actors::ListRooms)
                    .into_actor(self)
                    .then(|res, act, _| {
                        match res {
                            Ok(rooms) => {
                                act.framed.write(ChatResponse::Rooms(rooms));
                            }
                            _ => println!("Something is wrong"),
                        }
                        actix::fut::ready(())
                    })
                    .wait(ctx)
                // .wait(ctx) pauses all events in context,
                // so actor wont receive any new messages until it get list of rooms back
            }
            Ok(ChatRequest::Join(roomid)) => {
                println!("Join to room id: {}", roomid);
                self.room = Some(roomid);
                self.addr.do_send(ws_actors::Join {
                    session_id: self.id,
                    room_id: roomid,
                });
                self.framed.write(ChatResponse::Joined(roomid));
            }
            Ok(ChatRequest::Message(message)) => {
                // send message to chat server
                println!("Peer message: {}", message);
                if let Some(room) = self.room {
                    self.addr.do_send(ws_actors::Message {
                        id: self.id,
                        msg: message,
                        room: room,
                    })
                }
            }
            // we update heartbeat time on ping from peer
            Ok(ChatRequest::Ping) => self.hb = Instant::now(),
            _ => ctx.stop(),
        }
    }
}

/// Handler for Message, chat server sends this message, we just send string to
/// peer
impl Handler<Message> for ChatSession {
    type Result = ();

    fn handle(&mut self, msg: Message, _: &mut Context<Self>) {
        // send message to peer
        self.framed.write(ChatResponse::Message(msg.0));
    }
}

/// Helper methods
impl ChatSession {
    pub fn new(
        addr: Addr<ChatServer>,
        framed: actix::io::FramedWrite<ChatResponse, WriteHalf<TcpStream>, ChatCodec>,
    ) -> ChatSession {
        ChatSession {
            id: Uuid::new_v4(),
            addr,
            hb: Instant::now(),
            // defaultルームへの割り当てなし
            room: None,
            framed,
        }
    }

    /// helper method that sends ping to client every second.
    ///
    /// also this method check heartbeats from client
    fn hb(&self, ctx: &mut Context<Self>) {
        ctx.run_interval(Duration::new(1, 0), |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > Duration::new(10, 0) {
                // heartbeat timed out
                println!("Client heartbeat failed, disconnecting!");

                // notify chat server
                act.addr.do_send(ws_actors::Disconnect { id: act.id });

                // stop actor
                ctx.stop();
            }

            act.framed.write(ChatResponse::Ping);
            // if we can not send message to sink, sink is closed (disconnected)
        });
    }
}

/// Define tcp server that will accept incoming tcp connection and create
/// chat actors.
pub fn tcp_server(s: &str, server: Addr<ChatServer>) {
    // Create server listener
    let addr = net::SocketAddr::from_str(s).unwrap_or_else(|_| {
        panic!(
            "Invalid socket address: {}. Please check IP address or port number.",
            s
        )
    });

    actix_web::rt::spawn(async move {
        let server = server.clone();
        let mut listener = TcpListener::bind(&addr)
            .await
            .unwrap_or_else(|_| panic!("Cannot bind TCP listener to socket address: {}", &addr));
        let mut incoming = listener.incoming();

        while let Some(stream) = incoming.next().await {
            match stream {
                Ok(stream) => {
                    let server = server.clone();
                    // Create ChatSession Actor
                    ChatSession::create(|ctx| {
                        // Split TcpStream into ReadHalf and WriteHalf
                        let (r, w) = split(stream);
                        // Register tcp stream as reader to execution context for this ChatSession Actor
                        ChatSession::add_stream(FramedRead::new(r, ChatCodec), ctx);
                        // Register address of server actor with which this actor communicate and writer for this ChatSession Actor
                        ChatSession::new(server, actix::io::FramedWrite::new(w, ChatCodec, ctx))
                    });
                }
                Err(_) => return,
            }
        }
    });
}

struct WsChatSession {
    /// unique session id
    id: Uuid,
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    hb: Instant,
    /// joined room
    room: Option<Uuid>,
    /// peer name
    name: Option<String>,
    /// Chat server
    addr: Addr<ws_actors::ChatServer>,
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
            .send(ws_actors::Connect {
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
        self.addr.do_send(ws_actors::Disconnect { id: self.id });
        Running::Stop
    }
}

/// Handle messages from chat server, we simply send it to peer websocket
impl Handler<Message> for WsChatSession {
    type Result = ();

    fn handle(&mut self, msg: Message, ctx: &mut Self::Context) {
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
                                .send(ws_actors::ListRooms)
                                .into_actor(self)
                                .then(|res, _, ctx| {
                                    match res {
                                        Ok(rooms) => {
                                            ctx.text(rooms.get_json_data(
                                                ws_actors::Status::Ok,
                                                ws_actors::Event::GetRoomList,
                                            ));
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
                                    .send(ws_actors::Join {
                                        session_id: self.id,
                                        room_id: Uuid::parse_str(v[1]).unwrap(),
                                    })
                                    .into_actor(self)
                                    .then(|res, _, ctx| {
                                        match res {
                                            Ok(room_info) => {
                                                ctx.text(room_info.get_json_data(
                                                    ws_actors::Status::Ok,
                                                    ws_actors::Event::EnterRoom,
                                                ));
                                            }
                                            _ => println!("Something is wrong!"),
                                        }
                                        fut::ready(())
                                    })
                                    .wait(ctx)
                            } else {
                                ctx.text("!!! room id is required");
                            }
                        }
                        "/create" => {
                            if v.len() == 2 {
                                // self.room = v[1].to_owned();
                                self.addr
                                    .send(ws_actors::Create {
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
                                                ctx.text(data.get_json_data(
                                                    ws_actors::Status::Ok,
                                                    ws_actors::Event::CreateRoom,
                                                ));
                                            }
                                            // TODO: statusをerrorとして返した方がよい？
                                            _ => println!("Something is wrong"),
                                        }
                                        fut::ready(())
                                    })
                                    .wait(ctx)
                            } else {
                                ctx.text(
                                    ErrorMessage {
                                        message: "!!! room name is required".to_string(),
                                    }
                                    .get_json_data(
                                        ws_actors::Status::Error,
                                        ws_actors::Event::Unknown,
                                    ),
                                );
                            }
                        }
                        "/name" => {
                            if v.len() == 2 {
                                self.name = Some(v[1].to_owned());
                            } else {
                                ctx.text(
                                    ErrorMessage {
                                        message: "!!! name is required".to_string(),
                                    }
                                    .get_json_data(
                                        ws_actors::Status::Error,
                                        ws_actors::Event::Unknown,
                                    ),
                                );
                            }
                        }
                        _ => ctx.text(
                            ErrorMessage {
                                message: format!("!!! unknown command: {:?}", m),
                            }
                            .get_json_data(ws_actors::Status::Error, ws_actors::Event::Unknown),
                        ),
                    }
                } else {
                    let msg = if let Some(ref name) = self.name {
                        format!("{}: {}", name, m)
                    } else {
                        m.to_owned()
                    };
                    // send message to chat server
                    if let Some(room) = self.room {
                        self.addr.do_send(ws_actors::Message {
                            id: self.id,
                            msg,
                            room: room,
                        })
                    }
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
                act.addr.do_send(ws_actors::Disconnect { id: act.id });

                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }

            ctx.ping(b"");
        });
    }
}

/// Entry point for our websocket route
async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<ws_actors::ChatServer>>,
    // db_pool: web::Data<DbPool>,
) -> Result<HttpResponse, Error> {
    // print request headers
    for x in req.headers().iter() {
        println!("{:?}", x);
    }
    // start websocket
    ws::start(
        WsChatSession {
            id: Uuid::new_v4(),
            hb: Instant::now(),
            room: None, // defaultルームへの割り当てなし
            name: None,
            addr: srv.get_ref().clone(),
        },
        &req,
        stream,
    )
}

pub fn register(config: &mut web::ServiceConfig) {
    config.service(web::resource("/ws").to(ws_route));
}
