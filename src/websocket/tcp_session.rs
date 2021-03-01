use actix::prelude::*;
use futures::StreamExt;
use std::str::FromStr;
use std::{io, net};
use tokio::io::{split, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::FramedRead;
use uuid::Uuid;

use super::Message;
use super::*;

/// `ChatSession` actor is responsible for tcp peer communications.
pub struct ChatSession {
    /// unique session id
    id: Uuid,
    /// this is address of chat server
    addr: Addr<server::ChatServer>,
    /// Client must send ping at least once per 10 seconds, otherwise we drop
    /// connection.
    hb: Instant,
    /// joined room
    room: Option<Uuid>,
    /// Framed wrapper
    framed: actix::io::FramedWrite<codec::ChatResponse, WriteHalf<TcpStream>, codec::ChatCodec>,
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
                actix::fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        // notify chat server
        self.addr.do_send(Disconnect { id: self.id });
        Running::Stop
    }
}

impl actix::io::WriteHandler<io::Error> for ChatSession {}

/// To use `Framed` we have to define Io type and Codec
impl StreamHandler<Result<codec::ChatRequest, io::Error>> for ChatSession {
    /// This is main event loop for client requests
    fn handle(&mut self, msg: Result<codec::ChatRequest, io::Error>, ctx: &mut Context<Self>) {
        match msg {
            Ok(codec::ChatRequest::List) => {
                // Send ListRooms message to chat server and wait for response
                println!("List rooms");
                self.addr
                    .send(ListRooms)
                    .into_actor(self)
                    .then(|res, act, _| {
                        match res {
                            Ok(rooms) => {
                                act.framed.write(codec::ChatResponse::Rooms(rooms));
                            }
                            _ => println!("Something is wrong"),
                        }
                        actix::fut::ready(())
                    })
                    .wait(ctx)
                // .wait(ctx) pauses all events in context,
                // so actor wont receive any new messages until it get list of rooms back
            }
            Ok(codec::ChatRequest::Join(roomid)) => {
                println!("Join to room id: {}", roomid);
                self.room = Some(roomid);
                self.addr.do_send(Join {
                    session_id: self.id,
                    room_id: roomid,
                });
                self.framed.write(codec::ChatResponse::Joined(roomid));
            }
            Ok(codec::ChatRequest::Message(message)) => {
                // send message to chat server
                println!("Peer message: {}", message);
                if let Some(room) = self.room {
                    self.addr.do_send(Message {
                        id: self.id,
                        msg: message,
                        room: room,
                    })
                }
            }
            // we update heartbeat time on ping from peer
            Ok(codec::ChatRequest::Ping) => self.hb = Instant::now(),
            _ => ctx.stop(),
        }
    }
}

/// Handler for Message, chat server sends this message, we just send string to
/// peer
impl Handler<ChatMessage> for ChatSession {
    type Result = ();

    fn handle(&mut self, msg: ChatMessage, _: &mut Context<Self>) {
        // send message to peer
        self.framed.write(codec::ChatResponse::Message(msg.0));
    }
}

/// Helper methods
impl ChatSession {
    pub fn new(
        addr: Addr<server::ChatServer>,
        framed: actix::io::FramedWrite<codec::ChatResponse, WriteHalf<TcpStream>, codec::ChatCodec>,
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
                act.addr.do_send(Disconnect { id: act.id });

                // stop actor
                ctx.stop();
            }

            act.framed.write(codec::ChatResponse::Ping);
            // if we can not send message to sink, sink is closed (disconnected)
        });
    }
}

/// Define tcp server that will accept incoming tcp connection and create
/// chat actors.
pub fn tcp_server(s: &str, server: Addr<server::ChatServer>) {
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
                    println!("{:?}", stream);
                    let server = server.clone();
                    // Create ChatSession Actor
                    ChatSession::create(|ctx| {
                        // Split TcpStream into ReadHalf and WriteHalf
                        let (r, w) = split(stream);
                        // Register tcp stream as reader to execution context for this ChatSession Actor
                        ChatSession::add_stream(FramedRead::new(r, codec::ChatCodec), ctx);
                        // Register address of server actor with which this actor communicate and writer for this ChatSession Actor
                        ChatSession::new(
                            server,
                            actix::io::FramedWrite::new(w, codec::ChatCodec, ctx),
                        )
                    });
                }
                Err(_) => return,
            }
        }
    });
}
