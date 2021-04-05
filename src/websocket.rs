use std::{
    collections::HashSet,
    fmt::{Display, Formatter},
    time::{Duration, Instant},
};

use actix::prelude::*;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod codec;
pub mod room_manager;
pub mod tcp_session;
mod websocket_session;

/// Status list for websocket
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Status {
    Ok,
    Error,
}

impl Display for Status {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
        // or, alternatively:
        // fmt::Debug::fmt(self, f)
    }
}

/// Event list for websocket
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Event {
    /// event for creating room
    CreateRoom,
    /// event for entering room
    EnterRoom,
    /// event for getting room list
    GetRoomList,
    /// event for someone entering room
    SomeoneEnterRoom,
    /// event for receive cards info
    CardsInfo,
    /// unexpected event
    Unknown,
}

impl Display for Event {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
        // or, alternatively:
        // fmt::Debug::fmt(self, f)
    }
}

/// Message for chat server communications
/// New chat session is created
pub struct Connect {
    pub addr: Recipient<ChatMessage>,
}

impl actix::Message for Connect {
    type Result = Uuid;
}

/// Session is disconnected
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: Uuid,
}

/// Send message to specific room
#[derive(Message)]
#[rtype(result = "()")]
pub struct Message {
    /// Id of the client session
    pub id: Uuid,
    /// Peer message
    pub msg: String,
    /// Room id
    pub room: Uuid,
}

/// List of available rooms
pub struct ListRooms;

impl actix::Message for ListRooms {
    type Result = RoomInfoList;
}

/// Join room.
pub struct Join {
    /// Client id
    pub session_id: Uuid,
    /// Room id
    pub room_id: Uuid,
}

impl actix::Message for Join {
    type Result = RoomInfo;
}

pub struct Create {
    /// Client id
    pub session_id: Uuid,
    /// Room name
    pub room_name: String,
}

pub struct CreateRoom {
    pub room_id: Uuid,
    pub room_name: String,
}

impl actix::Message for Create {
    type Result = CreateRoom;
}

pub struct Session {
    address: Recipient<ChatMessage>,
}

pub struct Room {
    name: String,
    members: HashSet<Uuid>,
}

impl Room {
    fn remove_member(&mut self, session_id: &Uuid) -> bool {
        self.members.remove(session_id)
    }

    fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// Chat server sends this messages to session
#[derive(Message)]
#[rtype(result = "()")]
pub struct ChatMessage(pub String);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CardInfo {
    pub index: i32,
    pub is_own: bool,
    pub position_x: i32,
    pub position_y: i32,
    pub init_x: i32,
    pub init_y: i32,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CardInfoList {
    pub cards: Vec<CardInfo>,
}

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
pub struct SimpleMessage {
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsMessage<T> {
    data: T,
    event: Event,
    status: Status,
}

impl RoomInfo {
    pub fn get_json_data(&self, status: Status, event: Event) -> String {
        serde_json::to_string(&WsMessage {
            data: self.clone(),
            event,
            status,
        })
        .unwrap()
    }
}

impl RoomInfoList {
    pub fn get_json_data(&self, status: Status, event: Event) -> String {
        serde_json::to_string(&WsMessage {
            data: self.clone().rooms,
            event,
            status,
        })
        .unwrap()
    }
}

impl CardInfoList {
    pub fn get_json_data(&self, status: Status, event: Event) -> String {
        serde_json::to_string(&WsMessage {
            data: self.clone().cards,
            event,
            status,
        })
        .unwrap()
    }
}

impl SimpleMessage {
    pub fn get_json_data(&self, status: Status, event: Event) -> String {
        serde_json::to_string(&WsMessage {
            data: self.clone(),
            event,
            status,
        })
        .unwrap()
    }
}

/// Entry point for our websocket route
async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<room_manager::ChatServer>>,
    // db_pool: web::Data<DbPool>,
) -> Result<HttpResponse, Error> {
    // print request headers
    for x in req.headers().iter() {
        println!("{:?}", x);
    }
    // start websocket
    ws::start(
        websocket_session::WsChatSession {
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
