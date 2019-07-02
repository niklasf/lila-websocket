// -> site-in (to lila)
// <- site-out (from lila)
//
// example redis message (JSON):
//
// {
//   path: /connect
//   data: {
//     user: revoof
//   }
// }

// Global stuff:
// - Redis connection
// - By user id:
//   * number of connections (implied by list below)
//   * list of senders
// - By token:
//   * sender

use mongodb::ThreadedClient as _;
use mongodb::db::ThreadedDatabase as _;

use redis::Commands as _;

use cookie::Cookie;
use serde::{Serialize, Deserialize};
use serde_json::Value as JsonValue;

use ws::{Handshake, Handler, Frame, Sender, Message, CloseCode};
use ws::util::Token;
use mio_extras::timer::Timeout;

use std::collections::HashMap;
use std::str;
use std::sync::{Arc, Mutex};

#[derive(Serialize)]
#[serde(tag = "path")]
enum InternalMessage<'a> {
    #[serde(rename = "/connect")]
    Connect { user: &'a str },
    #[serde(rename = "/disconnect")]
    Disconnect { user: &'a str },
    #[serde(rename = "/notified")]
    Notified { user: &'a str },
}

#[derive(Deserialize)]
#[serde(tag = "path", content = "data")]
enum LilaMessage {
    #[serde(rename = "/move")]
    Move {
        #[serde(rename = "gameId")]
        game_id: String,
        fen: String,
        #[serde(rename = "move")]
        m: String,
    },
    #[serde(rename = "/tell/user")]
    Tell {
        user: String,
        payload: JsonValue,
    },
    #[serde(rename = "/tell/users")]
    TellMany {
        users: Vec<String>,
        payload: JsonValue,
    },
}

/// Messages received from the browser client, JSON encoded, over a Websocket.
#[derive(Deserialize)]
#[serde(tag = "t")]
enum ClientMessage {
    #[serde(rename = "p")]
    Ping { #[allow(unused)] l: u32 },
    #[serde(rename = "notified")]
    Notified,
}

const IDLE_TIMEOUT: Token = Token(1);

#[derive(Debug, Deserialize)]
struct SessionCookie {
    #[serde(rename = "sessionId")]
    session_id: String,
}

fn session_id(lila2: &str) -> Option<SessionCookie> {
    serde_urlencoded::from_str(lila2).ok()
}

fn user_id(cookie: &SessionCookie) -> Option<String> {
    let mut query = mongodb::Document::new();
    query.insert("_id", &cookie.session_id);

    // TODO: Currently making a new connection for each query.
    mongodb::Client::connect("127.0.0.1", 27017)
        .expect("mongodb connection")
        .db("lichess")
        .collection("security")
        .find_one(Some(query), None)
        .expect("query by sid")
        .and_then(|doc| doc.get_str("user").map(|s| s.to_owned()).ok())
}

struct App {
    // TODO: Find better datastructures, possibly lock-free.
    by_user: Mutex<HashMap::<String, Vec<Sender>>>,
    by_game: Mutex<HashMap::<String, Vec<Sender>>>,
    redis: Mutex<redis::Connection>,
}

impl App {
    fn new() -> App {
        let redis = redis::Client::open("redis://127.0.0.1/")
            .expect("redis open")
            .get_connection()
            .expect("redis connection");

        App {
            by_user: Mutex::new(HashMap::new()),
            by_game: Mutex::new(HashMap::new()),
            redis: Mutex::new(redis),
        }
    }

    fn publish(&self, msg: InternalMessage) {
        let mut guard = self.redis.lock().expect("redis");
        let con: &mut redis::Connection = &mut guard;
        let ret: u32 = con.publish("site-in", serde_json::to_string(&msg).expect("serialize")).expect("publish");
        if ret == 0 {
            println!("lila missed a publish");
        }
    }
}

fn main() {
    let app = Arc::new(App::new());

    ws::listen("127.0.0.1:9664", move |sender| {
        Server {
            app: app.clone(),
            sender,
            uid: None,
            watching: Vec::new(),
            idle_timeout: None
        }
    }).expect("ws listen");

    crossbeam::scope(|s| {
        s.spawn(|_| {
            let mut redis = redis::Client::open("redis://127.0.0.1/")
                .expect("redis open")
                .get_connection()
                .expect("redis connection");

            let mut incoming = redis.as_pubsub();
            incoming.subscribe("lila-out").expect("subscribe lila-out");

            loop {
                let msg = incoming.get_message().expect("incoming message");
                let payload: String = msg.get_payload().expect("payload");
            }
        });
    }).expect("scoped recv thread");
}

struct DefaultHandler;
impl Handler for DefaultHandler { }

struct Server {
    app: Arc<App>,
    sender: Sender,
    uid: Option<String>,
    watching: Vec<String>,
    idle_timeout: Option<Timeout>,
}

impl Handler for Server {
    fn on_open(&mut self, handshake: Handshake) -> ws::Result<()> {
        self.uid = handshake.request.header("cookie")
            .and_then(|h| str::from_utf8(h).ok())
            .and_then(|h| Cookie::parse(h).ok())
            .and_then(|c| {
                let (name, value) = c.name_value();
                Some(value.to_owned()).filter(|_| name == "lila2")
            })
            .and_then(|s| session_id(&s))
            .as_ref()
            .and_then(user_id);

        if let Some(ref uid) = self.uid {
            let mut by_user = self.app.by_user.lock().expect("lock by_user for open");
            by_user
                .entry(uid.to_owned())
                .and_modify(|v| v.push(self.sender.clone()))
                .or_insert_with(|| {
                    println!("connected: {}", uid);
                    self.app.publish(InternalMessage::Connect { user: uid });
                    vec![self.sender.clone()]
                });
        }

        self.sender.timeout(10_000, IDLE_TIMEOUT)
    }

    fn on_close(&mut self, _: CloseCode, _: &str) {
        if let Some(uid) = self.uid.take() {
            // Update by_user.
            let mut by_user = self.app.by_user.lock().expect("lock by_user for close");
            let entry = by_user.get_mut(&uid).expect("uid in map");
            let len_before = entry.len();
            entry.retain(|s| s.token() != self.sender.token());
            assert_eq!(entry.len() + 1, len_before);

            // Update by_game.
            let our_token = self.sender.token();
            let mut by_game = self.app.by_game.lock().expect("lock by_game for close");
            for game in self.watching.drain(..) {
                let watchers = by_game.get_mut(&game).expect("game in map");
                let len_before = watchers.len();
                watchers.retain(|s| s.token() != our_token);
                assert_eq!(watchers.len() + 1, len_before);
            }

            // Last remaining connection closed.
            if entry.is_empty() {
                by_user.remove(&uid);
                println!("disconnected: {}", uid);
                self.app.publish(InternalMessage::Disconnect { user: &uid });
            }
        }
    }

    fn on_message(&mut self, msg: Message) -> ws::Result<()> {
        if msg.as_text()? == "null" {
            // Fast path for ping.
            return self.sender.send(Message::text("0"));
        }

        match serde_json::from_str(msg.as_text()?) {
            Ok(ClientMessage::Ping { .. }) => {
                self.sender.send(Message::text("0"))
            }
            Ok(ClientMessage::Notified) => {
                if let Some(ref uid) = self.uid {
                    println!("notified: {}", uid);
                    self.app.publish(InternalMessage::Notified { user: uid });
                }
                Ok(())
            }
            Err(err) => {
                println!("protocol violation: {:?}", err);
                self.sender.close(CloseCode::Protocol)
            }
        }
    }

    fn on_new_timeout(&mut self, event: Token, timeout: Timeout) -> ws::Result<()> {
        assert_eq!(event, IDLE_TIMEOUT);
        if let Some(old_timeout) = self.idle_timeout.take() {
            self.sender.cancel(old_timeout)?;
        }
        self.idle_timeout = Some(timeout);
        Ok(())
    }

    fn on_timeout(&mut self, event: Token) -> ws::Result<()> {
        assert_eq!(event, IDLE_TIMEOUT);
        self.sender.close(CloseCode::Away)
    }

    fn on_frame(&mut self, frame: Frame) -> ws::Result<Option<Frame>> {
        self.sender.timeout(10_000, IDLE_TIMEOUT)?;
        DefaultHandler.on_frame(frame)
    }
}
