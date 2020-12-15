//! `GameServer` is an actor. It maintains list of connection client session.
//! And manages available rooms. Peers send messages to other peers in same
//! room through `GameServer`.

use actix::prelude::*;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use rand::{self, rngs::ThreadRng, Rng};
use std::collections::{HashMap, HashSet};

/// WebSocket用のアクターサーバーの構造体
/// `GameServer` manages chat rooms and responsible for coordinating chat
/// session. implementation is super primitive
pub struct WsActor {
    sessions: HashMap<u32, Recipient<Message>>,
    rooms: HashMap<String, HashSet<u32>>,
    rng: ThreadRng,
    visitor_count: Arc<AtomicUsize>,
}

impl WsActor {
    pub fn new(visitor_count: Arc<AtomicUsize>) -> WsActor {
        WsActor {
            sessions: HashMap::new(),
            rooms: HashMap::new(),
            rng: rand::thread_rng(),
            visitor_count,
        }
    }

    /// Send message to all users in the room
    fn send_message(&self, room: &str, message: WebsocketData) {
        if let Some(sessions) = self.rooms.get(room) {
            for id in sessions {
                if let Some(addr) = self.sessions.get(id) {
                    let _ = addr.do_send(Message(message));
                }
            }
        }
    }

    /// Send coordinate to all users in the room
    fn send_coordinate(&self, room: &str, coordinate: WebsocketData) {
        // TODO: self.rooms(sessionsのroomsではなく、dieaselからroomsをとる)
        if let Some(sessions) = self.rooms.get(room) {
            for id in sessions {
                if let Some(addr) = self.sessions.get(id) {
                    let _ = addr.do_send(Message(coordinate));
                }
            }
        }
    }

    /// Send client id to self user
    fn send_client_id(&self, client_id: WebsocketData) {
        for (_, addr) in &self.sessions {
            let _ = addr.do_send(Message(client_id));
        }
    }
}

/// Make actor from `GameServer`
impl Actor for WsActor {
    /// We are going to use simple Context, we just need ability to communicate
    /// with other actors.
    type Context = Context<Self>;
}

/// data via websocket
/// Coodinate(x, y) or message or client id
pub enum WebsocketData {
    Coordinate(i32, i32),
    Message(String),
    Id(u32),
}

/// メッセージ用の構造体
/// Game server sends this messages to session
#[derive(Message)]
#[rtype(result = "()")] // 処理を行った後にメインのサーバーに返すときの型を記述
pub struct Message(pub WebsocketData);

/// クライアントの接続をハンドルするときに使用
/// New game session is created
#[derive(Message)]
#[rtype(u32)]
pub struct Connect {
    pub addr: Recipient<Message>,
}

/// クライアントの切断をハンドルするときに使用
/// Session is disconnected
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: u32,
    pub room: String,
}

/// `Message`をハンドルするときに使用
/// Send message to specific room
#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientMessage {
    /// Id of the client session
    pub id: u32,
    /// Peer message
    pub data: WebsocketData,
    /// Room name
    pub room: String,
}

/// Handler for Connect message.
///
/// Register new session and assign unique id to this session
impl Handler<Connect> for WsActor {
    type Result = u32;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        println!("Someone joined");
        let client_id = rand::random::<u32>();
        // register session with random
        self.sessions.insert(client_id, msg.addr);
        // TODO: websocketハンドシェイクを先に行ってからidを得てGraphQLでidとroom名を追加する。
        self.send_client_id(WebsocketData::Id(client_id));

        // send id back
        client_id
    }
}

impl Handler<Disconnect> for WsActor {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        let client_id = msg.id;
        // TODO: room id の検索
        let room_name = msg.room;
        // TODO: send message to other users
        self.send_message(
            &room_name,
            WebsocketData::Message(format!("{} disconnected...;-;", client_id)),
        );
        // TODO: remove address
        self.sessions.remove(&client_id);
    }
}

impl Handler<ClientMessage> for WsActor {
    type Result = ();

    fn handle(&mut self, msg: ClientMessage, _: &mut Context<Self>) {
        // TODO: room id の検索
        let room_name = msg.room;
        self.send_coordinate(&room_name, msg.data);
    }
}
