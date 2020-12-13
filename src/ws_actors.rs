use actix::prelude::*;

use std::collections::HashMap;

/// WebSocket用のアクターサーバーの構造体
pub struct WsActor {
    sessions: HashMap<u32, Recipient<Message>>,
}

impl WsActor {
    pub fn new() -> WsActor {
        WsActor {
            sessions: HashMap::new(),
        }
    }

    fn send_message(&self, message: &str) {
        for (_, addr) in &self.sessions {
            let _ = addr.do_send(Message(message.to_owned()));
        }
    }
}

impl Actor for WsActor {
    type Context = Context<Self>;
}

/// メッセージ用の構造体
#[derive(Message)]
#[rtype(result = "()")] // 処理を行った後にメインのサーバーに返すときの型を記述
pub struct Message(pub String);

/// クライアントの接続をハンドルするときに使用
#[derive(Message)]
#[rtype(u32)]
pub struct Connect {
    pub addr: Recipient<Message>,
}

/// クライアントの切断をハンドルするときに使用
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: u32,
}

/// `Message`をハンドルするときに使用
#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientMessage {
    pub id: u32,
    pub msg: String,
}

impl Handler<Connect> for WsActor {
    type Result = u32;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        let client_id = rand::random::<u32>();
        self.sessions.insert(client_id, msg.addr);
        self.send_message(&format!("{} connected!", client_id));
        client_id
    }
}

impl Handler<Disconnect> for WsActor {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        let client_id = msg.id;
        self.send_message(&format!("{} disconnected...;-;", client_id));
        self.sessions.remove(&client_id);
    }
}

impl Handler<ClientMessage> for WsActor {
    type Result = ();

    fn handle(&mut self, msg: ClientMessage, _: &mut Context<Self>) {
        self.send_message(&msg.msg);
    }
}
