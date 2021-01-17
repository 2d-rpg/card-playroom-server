//! `ChatServer` is an actor. It maintains list of connection client session.
//! And manages available rooms. Peers send messages to other peers in same
//! room through `ChatServer`.

use actix::prelude::*;
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
    type Result = Vec<ws_session::RoomInfo>;
}

/// Join room, if room does not exists create new one.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Join {
    /// Client id
    pub session_id: Uuid,
    /// Room id
    pub room_id: Uuid,
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

pub struct Room {
    name: String,
    users: HashSet<Uuid>,
}

pub struct Session {
    address: Recipient<ws_session::Message>,
}

/// `ChatServer` manages chat rooms and responsible for coordinating chat
/// session. implementation is super primitive
pub struct ChatServer {
    sessions: HashMap<Uuid, Session>,
    rooms: HashMap<Uuid, Room>,
}

impl Default for ChatServer {
    fn default() -> ChatServer {
        // TODO default room 必要？
        // let mut rooms = HashMap::new();
        // rooms.insert(
        //     Uuid::new_v4(),
        //     Room {
        //         name: "メインルーム(デフォルト)".to_owned(),
        //         users: HashSet::new(),
        //     },
        // );

        ChatServer {
            sessions: HashMap::new(),
            rooms: HashMap::new(),
        }
    }
}

impl ChatServer {
    /// Send message to all users in the room
    fn send_message(&self, room: &Uuid, message: &str, skip_id: Uuid) {
        if let Some(Room { name: _, users }) = self.rooms.get(room) {
            for id in users {
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

    fn add_room(&mut self, session_id: &Uuid, room_name: &str) -> MessageResult<Create> {
        self.rooms.insert(
            session_id.clone(), // room id becomes room host session id
            Room {
                name: room_name.to_owned(),
                users: HashSet::new(),
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

        let mut rooms: Vec<&Uuid> = Vec::new();

        // remove address
        if self.sessions.remove(&msg.id).is_some() {
            // remove session from all rooms
            for (id, room) in &mut self.rooms {
                if room.users.remove(&msg.id) {
                    rooms.push(id);
                }
            }
        }
        // TODO if a room host is disconnected, non-host member should close websocket
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

        for (id, room) in &self.rooms {
            let room = ws_session::RoomInfo {
                id: *id,
                name: room.name.to_owned(),
                num: room.users.len(),
            };
            rooms.push(room);
        }

        MessageResult(rooms)
    }
}

/// Join room, send disconnect message to old room
/// send join message to new room
impl Handler<Join> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Join, _: &mut Context<Self>) {
        let Join {
            session_id,
            room_id,
        } = msg;
        // let mut rooms = Vec::new();

        // remove session from all rooms
        // for (room_id, sessions) in &mut self.roomusers {
        //     if sessions.remove(&id) {
        //         rooms.push(room_id);
        //     }
        // }
        // send message to other users
        // for room in rooms {
        //     self.send_message(&room, "Someone disconnected", 0);
        // }

        // create room if the named room does not exist
        // if self.roomnames.get_mut(&roomid).is_none() {
        //     let roomid = self.rng.gen::<usize>();
        //     self.roomnames.insert(name.clone(), roomid);
        //     self.roomusers.insert(roomid, HashSet::new());
        // }

        // send all users in the room except self
        self.send_message(&room_id, "Someone connected", session_id);
        // add session id
        self.rooms
            .get_mut(&room_id)
            .unwrap()
            .users
            .insert(session_id);
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
