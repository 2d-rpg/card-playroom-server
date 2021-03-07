use std::collections::HashMap;

use super::Message;
use super::*;

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
                if *id != skip_id {
                    if let Some(Session { address }) = self.sessions.get(id) {
                        let _ = address.do_send(ChatMessage(message.to_owned()));
                    }
                }
            }
        }
    }

    fn send_all(&self, message: &str) {
        for (_session_id, session) in &self.sessions {
            let _ = session.address.do_send(ChatMessage(message.to_owned()));
        }
    }

    fn update_room_list(&self) {
        let mut rooms = Vec::new();

        // Get room list
        for (room_id, Room { name, members }) in &self.rooms {
            let room = RoomInfo {
                id: room_id.clone(),
                name: name.to_owned(),
                num: members.len(),
            };
            rooms.push(room);
        }
        self.send_all(&RoomInfoList { rooms: rooms }.get_json_data(Status::Ok, Event::GetRoomList));
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

    fn add_session(&mut self, address: Recipient<ChatMessage>) -> Uuid {
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
            let room = RoomInfo {
                id: room_id.clone(),
                name: name.to_owned(),
                num: members.len(),
            };
            rooms.push(room);
        }

        MessageResult(RoomInfoList { rooms })
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
        MessageResult(RoomInfo {
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
