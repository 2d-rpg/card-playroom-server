//! `ChatServer` is an actor. It maintains list of connection client session.
//! And manages available rooms. Peers send messages to other peers in same
//! room through `ChatServer`.

use actix::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::ws_session;

/// Message for chat server communications

/// New chat session is created
pub struct Connect {
    pub addr: Recipient<ws_session::Message>,
}

impl actix::Message for Connect {
    type Result = Uuid;
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Status {
    Ok,
    Error,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
        // or, alternatively:
        // fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Event {
    CreateRoom,
    EnterRoom,
    GetRoomList,
    Unknown,
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
        // or, alternatively:
        // fmt::Debug::fmt(self, f)
    }
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
    type Result = ws_session::RoomInfoList;
}

/// Join room.
pub struct Join {
    /// Client id
    pub session_id: Uuid,
    /// Room id
    pub room_id: Uuid,
}

impl actix::Message for Join {
    type Result = ws_session::RoomInfo;
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
    address: Recipient<ws_session::Message>,
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

/// `ChatServer` manages chat rooms and responsible for coordinating chat
/// session. implementation is super primitive
pub struct ChatServer {
    sessions: HashMap<Uuid, Session>,
    rooms: HashMap<Uuid, Room>,
}

impl Default for ChatServer {
    fn default() -> ChatServer {
        ChatServer {
            sessions: HashMap::new(),
            rooms: HashMap::new(),
        }
    }
}

impl ChatServer {
    /// Send message to all users in the room
    fn send_message(&self, room: &Uuid, message: &str, skip_id: Uuid) {
        if let Some(Room { name: _, members }) = self.rooms.get(room) {
            for id in members {
                println!("{}", id);
                if *id != skip_id {
                    if let Some(Session { address }) = self.sessions.get(id) {
                        println!("{:?}", address);
                        let _ = address.do_send(ws_session::Message(message.to_owned()));
                    }
                }
            }
        }
    }

    fn send_all(&self, message: &str) {
        for (_session_id, session) in &self.sessions {
            let _ = session
                .address
                .do_send(ws_session::Message(message.to_owned()));
        }
    }

    fn update_room_list(&self) {
        let mut rooms = Vec::new();

        // Get room list
        for (room_id, Room { name, members }) in &self.rooms {
            let room = ws_session::RoomInfo {
                id: room_id.clone(),
                name: name.to_owned(),
                num: members.len(),
            };
            rooms.push(room);
        }
        self.send_all(
            &ws_session::RoomInfoList { rooms: rooms }
                .get_json_data(Status::Ok, Event::GetRoomList),
        );
    }

    fn add_room(&mut self, session_id: &Uuid, room_name: &str) -> MessageResult<Create> {
        self.rooms.insert(
            // room id becomes room host session id
            session_id.clone(),
            Room {
                name: room_name.to_owned(),
                members: HashSet::new(),
            },
        );
        MessageResult(CreateRoom {
            room_id: session_id.clone(),
            room_name: room_name.to_owned(),
        })
    }

    fn remove_room(&mut self, room_id: &Uuid) {
        self.rooms.remove(room_id);
    }

    fn add_session(&mut self, address: Recipient<ws_session::Message>) -> Uuid {
        let session_id = Uuid::new_v4();
        self.sessions
            .insert(session_id, Session { address: address });
        session_id
    }

    fn remove_session(&mut self, msg: &Disconnect) -> Vec<Uuid> {
        let mut rooms: Vec<Uuid> = Vec::new();
        if self.sessions.remove(&msg.id).is_some() {
            // remove session from all rooms
            for (id, room) in &mut self.rooms {
                if room.remove_member(&msg.id) {
                    if room.is_empty() {
                        rooms.push(id.clone());
                    }
                }
            }
        }
        rooms
    }
}

/// Make actor from `ChatServer`
impl Actor for ChatServer {
    /// We are going to use simple Context, we just need ability to communicate
    /// with other actors.
    type Context = Context<Self>;
}

/// Handler for Connect message.
///
/// Register new session and assign unique id to this session
impl Handler<Connect> for ChatServer {
    type Result = MessageResult<Connect>;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        println!("Someone joined");

        // register session with random id
        MessageResult(self.add_session(msg.addr))
    }
}

/// Handler for Disconnect message.
impl Handler<Disconnect> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        println!("Someone disconnected");

        // remove address
        // if a room host is disconnected, non-host member should close websocket
        let room_ids = self.remove_session(&msg);
        for room_id in room_ids {
            self.remove_room(&room_id);
        }
        self.update_room_list();
    }
}

/// Handler for Message message.
impl Handler<Message> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Message, _: &mut Context<Self>) {
        self.send_message(&msg.room, msg.msg.as_str(), msg.id);
    }
}

/// Handler for `ListRooms` message.
impl Handler<ListRooms> for ChatServer {
    type Result = MessageResult<ListRooms>;

    fn handle(&mut self, _: ListRooms, _: &mut Context<Self>) -> Self::Result {
        let mut rooms = Vec::new();

        for (room_id, Room { name, members }) in &self.rooms {
            let room = ws_session::RoomInfo {
                id: room_id.clone(),
                name: name.to_owned(),
                num: members.len(),
            };
            rooms.push(room);
        }

        MessageResult(ws_session::RoomInfoList { rooms })
    }
}

/// Join room, send disconnect message to old room
/// send join message to new room
impl Handler<Join> for ChatServer {
    type Result = MessageResult<Join>;

    fn handle(&mut self, msg: Join, _: &mut Context<Self>) -> Self::Result {
        let Join {
            session_id,
            room_id,
        } = msg;

        // send all users in the room except self
        self.send_message(&room_id, "Someone connected", session_id);
        // add session id
        self.rooms
            .get_mut(&room_id)
            // TODO Result
            .unwrap()
            .members
            .insert(session_id);
        MessageResult(ws_session::RoomInfo {
            id: room_id,
            name: self.rooms.get(&room_id).unwrap().name.clone(),
            num: self.rooms.get(&room_id).unwrap().members.len(),
        })
    }
}

impl Handler<Create> for ChatServer {
    type Result = MessageResult<Create>;

    fn handle(&mut self, msg: Create, _: &mut Context<Self>) -> Self::Result {
        let Create {
            session_id,
            room_name,
        } = msg;

        self.add_room(&session_id, &room_name)
    }
}
