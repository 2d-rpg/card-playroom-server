//! `ChatServer` is an actor. It maintains list of connection client session.
//! And manages available rooms. Peers send messages to other peers in same
//! room through `ChatServer`.

use actix::prelude::*;
use rand::{self, rngs::ThreadRng, Rng};
use std::collections::{HashMap, HashSet};

use crate::ws_session;

/// Message for chat server communications

/// New chat session is created
#[derive(Message)]
#[rtype(usize)]
pub struct Connect {
    pub addr: Recipient<ws_session::Message>,
}

/// Session is disconnected
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: usize,
}

/// Send message to specific room
#[derive(Message)]
#[rtype(result = "()")]
pub struct Message {
    /// Id of the client session
    pub id: usize,
    /// Peer message
    pub msg: String,
    /// Room id
    pub room: usize,
}

/// List of available rooms
pub struct ListRooms;

impl actix::Message for ListRooms {
    type Result = Vec<ws_session::Room>;
}

/// Join room, if room does not exists create new one.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Join {
    /// Client id
    pub id: usize,
    /// Room id
    pub roomid: usize,
}

pub struct Create {
    /// Client id
    pub id: usize,
    /// Room name
    pub roomname: String,
}

pub struct CreateRoom {
    pub id: usize,
    pub roomname: String,
    pub exists: bool,
}

impl actix::Message for Create {
    type Result = CreateRoom;
}

/// `ChatServer` manages chat rooms and responsible for coordinating chat
/// session. implementation is super primitive
pub struct ChatServer {
    sessions: HashMap<usize, Recipient<ws_session::Message>>,
    roomusers: HashMap<usize, HashSet<usize>>,
    roomnames: HashMap<String, usize>,
    rng: ThreadRng,
}

impl Default for ChatServer {
    fn default() -> ChatServer {
        // default room
        let mut roomusers = HashMap::new();
        let mut roomnames = HashMap::new();
        let mut rng = rand::thread_rng();
        let main_room_id = rng.gen::<usize>();
        roomusers.insert(main_room_id, HashSet::new());
        roomnames.insert("Main".to_owned(), main_room_id);

        ChatServer {
            sessions: HashMap::new(),
            roomusers: roomusers,
            roomnames: roomnames,
            rng: rng,
        }
    }
}

impl ChatServer {
    /// Send message to all users in the room
    fn send_message(&self, room: &usize, message: &str, skip_id: usize) {
        if let Some(sessions) = self.roomusers.get(room) {
            for id in sessions {
                println!("{}", id);
                if *id != skip_id {
                    if let Some(addr) = self.sessions.get(id) {
                        println!("{:?}", addr);
                        let _ = addr.do_send(ws_session::Message(message.to_owned()));
                    }
                }
            }
        }
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
    type Result = usize;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        println!("Someone joined");

        // notify all users in same room
        self.send_message(
            self.roomnames.get(&"Main".to_owned()).unwrap(),
            "Someone joined",
            0,
        );

        // register session with random id
        let id = self.rng.gen::<usize>();
        self.sessions.insert(id, msg.addr);

        // auto join session to Main room
        self.roomusers
            .get_mut(self.roomnames.get(&"Main".to_owned()).unwrap())
            .unwrap()
            .insert(id);

        // send id back
        id
    }
}

/// Handler for Disconnect message.
impl Handler<Disconnect> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        println!("Someone disconnected");

        let mut rooms: Vec<&usize> = Vec::new();

        // remove address
        if self.sessions.remove(&msg.id).is_some() {
            // remove session from all rooms
            for (roomid, sessions) in &mut self.roomusers {
                if sessions.remove(&msg.id) {
                    rooms.push(roomid);
                }
            }
        }
        // send message to other users
        // for room in rooms {
        //     self.send_message(&room, "Someone disconnected", 0);
        // }
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

        for (name, id) in &self.roomnames {
            let room = ws_session::Room {
                id: *id,
                name: name.clone(),
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
        let Join { id, roomid } = msg;
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
        self.send_message(&roomid, "Someone connected", id);
        println!("{:?}", self.roomnames);
        println!("{:?}", self.roomusers);
        // add session id
        self.roomusers.get_mut(&roomid).unwrap().insert(id);
    }
}

impl Handler<Create> for ChatServer {
    type Result = MessageResult<Create>;

    fn handle(&mut self, msg: Create, _: &mut Context<Self>) -> Self::Result {
        let Create { id, roomname } = msg;

        if self.roomnames.get_mut(&roomname).is_none() {
            // create room if the named room does not exist
            let roomid = self.rng.gen::<usize>();
            self.roomnames.insert(roomname.clone(), roomid);
            self.roomusers.insert(roomid, HashSet::new());
            // self.roomusers.get_mut(&roomid).unwrap().insert(id);
            return MessageResult(CreateRoom {
                id: roomid,
                roomname: roomname,
                exists: false,
            });
        } else {
            // return error if already exists
            return MessageResult(CreateRoom {
                id: *self.roomnames.get(&roomname).unwrap(),
                roomname: roomname,
                exists: true,
            });
        }
    }
}
