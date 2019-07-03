// Before production ready:
// - Decide if nginx should be involved
// - Ready on lila's side?
// - Communicate count of online players

use mongodb::ThreadedClient as _;
use mongodb::db::ThreadedDatabase as _;
use mongodb::coll::options::FindOptions;
use bson::{doc, bson};

use redis::Commands as _;

use cookie::Cookie;
use serde::{Serialize, Deserialize};
use serde_json::Value as JsonValue;

use ws::{Handshake, Handler, Frame, Sender, Message, CloseCode};
use ws::util::Token;
use mio_extras::timer::Timeout;

use std::collections::{HashMap, HashSet};
use std::str;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU32, Ordering};

/// Messages we send to lila.
#[derive(Serialize)]
#[serde(tag = "path")]
enum LilaIn<'a> {
    #[serde(rename = "/connect")]
    Connect { user: &'a str },
    #[serde(rename = "/disconnect")]
    Disconnect { user: &'a str },
    #[serde(rename = "/notified")]
    Notified { user: &'a str },
    #[serde(rename = "/watch")]
    Watch { game: &'a str },
}

/// Messages we receive from lila.
#[derive(Deserialize)]
#[serde(tag = "path", content = "data")]
enum LilaOut {
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

/// Messages we send to Websocket clients.
#[derive(Serialize)]
#[serde(tag = "t", content = "d")]
enum SocketIn<'a> {
    #[serde(rename = "fen")]
    Fen {
        id: &'a str,
        fen: &'a str,
        lm: &'a str,
    }
}

/// Messages we receive from Websocket clients.
#[derive(Deserialize)]
#[serde(tag = "t")]
enum SocketOut {
    #[serde(rename = "p")]
    Ping { #[allow(unused)] l: u32 },
    #[serde(rename = "notified")]
    Notified,
    #[serde(rename = "startWatching")]
    StartWatching { d: String },
}

/// Session cookie from Play framework.
#[derive(Debug, Deserialize)]
struct SessionCookie {
    #[serde(rename = "sessionId")]
    session_id: String,
}

/// Token for the timeout that's used to closed Websockets after some time
/// of inactivity.
const IDLE_TIMEOUT: Token = Token(1);

/// State of this Websocket server.
struct App {
    by_user: RwLock<HashMap::<String, Vec<Sender>>>,
    by_game: RwLock<HashMap::<String, Vec<Sender>>>,
    redis_sink: crossbeam::channel::Sender<String>,
    session_store: mongodb::coll::Collection,
    connection_count: AtomicU32,
}

impl App {
    fn new(redis_sink: crossbeam::channel::Sender<String>, session_store: mongodb::coll::Collection) -> App {
        App {
            by_user: RwLock::new(HashMap::new()),
            by_game: RwLock::new(HashMap::new()),
            redis_sink,
            session_store,
            connection_count: AtomicU32::new(0),
        }
    }

    fn publish(&self, msg: LilaIn) {
        self.redis_sink.send(serde_json::to_string(&msg).expect("serialize")).expect("redis sink");
    }

    fn received(&self, msg: LilaOut) {
        match msg {
            LilaOut::Tell { user, payload } => {
                let by_user = self.by_user.read().expect("by_user for tell");
                if let Some(entry) = by_user.get(&user) {
                    for sender in entry {
                        if let Err(err) = sender.send(Message::text(payload.to_string())) {
                            log::warn!("failed to tell ({}): {:?}", user, err);
                        }
                    }
                }
            }
            LilaOut::TellMany { users, payload } => {
                let by_user = self.by_user.read().expect("by_user for tell many");
                for user in &users {
                    if let Some(entry) = by_user.get(user) {
                        for sender in entry {
                            if let Err(err) = sender.send(Message::text(payload.to_string())) {
                                log::warn!("failed to tell ({}): {:?}", user, err);
                            }
                        }
                    }
                }
            }
            LilaOut::Move { ref game_id, ref fen, ref m } => {
                let by_game = self.by_game.read().expect("by_game for move");
                if let Some(entry) = by_game.get(game_id) {
                    let msg = Message::text(serde_json::to_string(&SocketIn::Fen {
                        id: game_id,
                        fen,
                        lm: m,
                    }).expect("serialize fen"));

                    for sender in entry {
                        if let Err(err) = sender.send(msg.clone()) {
                            log::warn!("failed to send fen: {:?}", err);
                        }
                    }
                }
            }
        }
    }
}

/// A Websocket client connection.
struct Socket {
    app: &'static App,
    sender: Sender,
    uid: Option<String>,
    watching: HashSet<String>,
    idle_timeout: Option<Timeout>,
}

impl Handler for Socket {
    fn on_open(&mut self, handshake: Handshake) -> ws::Result<()> {
        // Inc connection count. (To avoid overflow, the corresponding
        // decrement needs to be ordered after this.)
        self.app.connection_count.fetch_add(1, Ordering::SeqCst);

        // Ask mongodb for user id based on session cookie.
        self.uid = handshake.request.header("cookie")
            .and_then(|h| str::from_utf8(h).ok())
            .and_then(|h| Cookie::parse(h).ok())
            .and_then(|c| {
                let (name, value) = c.name_value();
                Some(value.to_owned()).filter(|_| name == "lila2")
            })
            .and_then(|s| serde_urlencoded::from_str::<SessionCookie>(&s).ok())
            .and_then(|c| {
                let query = doc! { "_id": c.session_id, "up": true, };
                let mut opts = FindOptions::new();
                opts.projection = Some(doc! { "user": true });
                match self.app.session_store.find_one(Some(query), Some(opts)) {
                    Ok(Some(doc)) => doc.get_str("user").map(|s| s.to_owned()).ok(),
                    Ok(None) => {
                        log::info!("session store lookup with expired sid");
                        None
                    }
                    Err(err) => {
                        log::error!("session store query failed: {:?}", err);
                        None
                    }
                }
            });

        // Add socket to by_user map.
        if let Some(ref uid) = self.uid {
            let mut by_user = self.app.by_user.write().expect("lock by_user for open");
            by_user
                .entry(uid.to_owned())
                .and_modify(|v| v.push(self.sender.clone()))
                .or_insert_with(|| {
                    log::debug!("first open: {}", uid);
                    self.app.publish(LilaIn::Connect { user: uid });
                    vec![self.sender.clone()]
                });
        }

        // Start idle timeout.
        self.sender.timeout(10_000, IDLE_TIMEOUT)
    }

    fn on_close(&mut self, _: CloseCode, _: &str) {
        self.app.connection_count.fetch_sub(1, Ordering::SeqCst);

        if let Some(uid) = self.uid.take() {
            // Update by_user.
            let mut by_user = self.app.by_user.write().expect("lock by_user for close");
            let entry = by_user.get_mut(&uid).expect("uid in map");
            let idx = entry.iter().position(|s| s.token() == self.sender.token()).expect("uid in by_user");
            entry.swap_remove(idx);


            // Update by_game.
            let our_token = self.sender.token();
            let mut by_game = self.app.by_game.write().expect("lock by_game for close");
            for game in self.watching.drain() {
                let watchers = by_game.get_mut(&game).expect("game in map");
                let idx = watchers.iter().position(|s| s.token() == our_token).expect("sender in watchers");
                watchers.swap_remove(idx);
                if watchers.is_empty() {
                    by_game.remove(&game);
                }
            }

            // Last remaining connection closed.
            // TODO: Can we use entry API here?
            if entry.is_empty() {
                by_user.remove(&uid);
                log::debug!("last close: {}", uid);
                self.app.publish(LilaIn::Disconnect { user: &uid });
            }
        }
    }

    fn on_message(&mut self, msg: Message) -> ws::Result<()> {
        if msg.as_text()? == "null" {
            // Fast path for ping.
            return self.sender.send(Message::text("0"));
        }

        match serde_json::from_str(msg.as_text()?) {
            Ok(SocketOut::Ping { .. }) => {
                self.sender.send(Message::text("0"))
            }
            Ok(SocketOut::Notified) => {
                if let Some(ref uid) = self.uid {
                    log::debug!("notified: {}", uid);
                    self.app.publish(LilaIn::Notified { user: uid });
                }
                Ok(())
            }
            Ok(SocketOut::StartWatching { d }) => {
                log::debug!("start watching: {}", d);
                self.app.publish(LilaIn::Watch { game: &d });
                Ok(())
            },
            Err(err) => {
                log::warn!("protocol violation of client: {:?}", err);
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
        log::info!("closing socket due to timeout");
        self.sender.close(CloseCode::Away)
    }

    fn on_frame(&mut self, frame: Frame) -> ws::Result<Option<Frame>> {
        self.sender.timeout(10_000, IDLE_TIMEOUT)?;
        DefaultHandler.on_frame(frame)
    }
}

/// Used to get the normal `on_frame` behavior.
struct DefaultHandler;
impl Handler for DefaultHandler { }

fn main() {
    env_logger::init();

    crossbeam::scope(|s| {
        let session_store = mongodb::Client::connect("127.0.0.1", 27017)
            .expect("mongodb connect")
            .db("lichess")
            .collection("security");

        let (redis_sink, redis_recv) = crossbeam::channel::unbounded();
        let app: &'static App = Box::leak(Box::new(App::new(redis_sink, session_store)));

        // Thread for outgoing messages to lila.
        s.spawn(move |_| {
            let redis = redis::Client::open("redis://127.0.0.1/")
                .expect("redis open for publish")
                .get_connection()
                .expect("redis connection for publish");

            loop {
                let msg = redis_recv.recv().expect("redis recv");
                let ret: u32 = redis.publish("site-in", msg).expect("publish site-in");
                if ret == 0 {
                    log::error!("lila missed as message");
                }
            }
        });

        // Thread for incoming messages from lila.
        s.spawn(move |_| {
            let mut redis = redis::Client::open("redis://127.0.0.1/")
                .expect("redis open for subscribe")
                .get_connection()
                .expect("redis connection for subscribe");

            let mut incoming = redis.as_pubsub();
            incoming.subscribe("lila-out").expect("subscribe lila-out");

            loop {
                let redis_msg = incoming.get_message().expect("get message");
                let payload: String = redis_msg.get_payload().expect("get payload");
                let msg: LilaOut = serde_json::from_str(&payload).expect("lila out");
                app.received(msg);
            }
        });

        ws::listen("127.0.0.1:9664", move |sender| {
            Socket {
                app,
                sender,
                uid: None,
                watching: HashSet::new(),
                idle_timeout: None
            }
        }).expect("ws listen");
    }).expect("scope");
}
