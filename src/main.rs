use mongodb::ThreadedClient as _;
use mongodb::db::ThreadedDatabase as _;
use mongodb::coll::options::FindOptions;
use bson::{doc, bson};

use redis::Commands as _;

use cookie::Cookie;
use serde::{Serialize, Deserialize};
use serde_json::Value as JsonValue;

use ws::{Handshake, Handler, Sender, Message, CloseCode};
use ws::util::Token;
use mio_extras::timer::Timeout;

use std::collections::{HashMap, HashSet};
use std::str;
use std::sync::RwLock;
use once_cell::sync::OnceCell;

/// Messages we send to lila.
#[derive(Serialize)]
#[serde(tag = "path")]
enum LilaIn {
    #[serde(rename = "connect")]
    Connect { user: String },
    #[serde(rename = "disconnect")]
    Disconnect { user: String },
    #[serde(rename = "notified")]
    Notified { user: String },
    #[serde(rename = "watch")]
    Watch { game: String },
    Inc, // updates counter
    Dec, // updates counter
}

/// Messages we receive from lila.
#[derive(Deserialize)]
#[serde(tag = "path")]
enum LilaOut {
    #[serde(rename = "move")]
    Move {
        #[serde(rename = "gameId")]
        game_id: String,
        fen: String,
        #[serde(rename = "move")]
        m: String,
    },
    #[serde(rename = "tell/user")]
    Tell {
        user: String,
        payload: JsonValue,
    },
    #[serde(rename = "tell/users")]
    TellMany {
        users: Vec<String>,
        payload: JsonValue,
    },
    #[serde(rename = "tell/all")]
    TellAll {
        payload: JsonValue,
    }
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
    redis_sink: crossbeam::channel::Sender<LilaIn>,
    session_store: mongodb::coll::Collection,
    broadcaster: OnceCell<Sender>,
}

impl App {
    fn new(redis_sink: crossbeam::channel::Sender<LilaIn>, session_store: mongodb::coll::Collection) -> App {
        App {
            by_user: RwLock::new(HashMap::new()),
            by_game: RwLock::new(HashMap::new()),
            redis_sink,
            session_store,
            broadcaster: OnceCell::new(),
        }
    }

    fn publish(&self, msg: LilaIn) {
        self.redis_sink.send(msg).expect("redis sink");
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
            LilaOut::TellAll { payload } => {
                let msg = serde_json::to_string(&payload).expect("serialize broadcast");
                if let Err(err) = self.broadcaster.get().expect("broadcaster").send(msg) {
                    log::warn!("failed to send broadcast: {:?}", err);
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
        // Update connection count.
        self.app.publish(LilaIn::Inc);

        // Ask mongodb for user id based on session cookie.
        self.uid = handshake.request.header("cookie")
            .and_then(|h| str::from_utf8(h).ok())
            .and_then(|h| {
                h.split(';')
                    .map(|p| p.trim())
                    .filter(|p| p.starts_with("lila2="))
                    .next()
            })
            .and_then(|h| Cookie::parse(h).ok())
            .map(|c| dbg!(c))
            .and_then(|c| {
                let s = c.value();
                let idx = s.find('-').map(|n| n + 1).unwrap_or(0);
                serde_urlencoded::from_str::<SessionCookie>(&s[idx..]).ok()
            })
            .map(|d| dbg!(d))
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
                    self.app.publish(LilaIn::Connect { user: uid.to_owned() });
                    vec![self.sender.clone()]
                });
        }

        // Start idle timeout.
        self.sender.timeout(10_000, IDLE_TIMEOUT)
    }

    fn on_close(&mut self, _: CloseCode, _: &str) {
        // Update connection count.
        self.app.publish(LilaIn::Dec);

        // Clear timeout.
        if let Some(timeout) = self.idle_timeout.take() {
            if let Err(err) = self.sender.cancel(timeout) {
                log::error!("failed to clear timeout: {:?}", err);
            }
        }

        // Update by_user.
        if let Some(uid) = self.uid.take() {
            let mut by_user = self.app.by_user.write().expect("lock by_user for close");
            let entry = by_user.get_mut(&uid).expect("uid in map");
            let idx = entry.iter().position(|s| s.token() == self.sender.token()).expect("uid in by_user");
            entry.swap_remove(idx);

            // Last remaining connection closed.
            if entry.is_empty() {
                by_user.remove(&uid);
                log::debug!("last close: {}", uid);
                self.app.publish(LilaIn::Disconnect { user: uid });
            }
        }

        // Update by_game.
        let mut by_game = self.app.by_game.write().expect("lock by_game for close");
        let our_token = self.sender.token();
        for game in self.watching.drain() {
            let watchers = by_game.get_mut(&game).expect("game in map");
            let idx = watchers.iter().position(|s| s.token() == our_token).expect("sender in watchers");
            watchers.swap_remove(idx);
            if watchers.is_empty() {
                by_game.remove(&game);
            }
        }
    }

    fn on_message(&mut self, msg: Message) -> ws::Result<()> {
        self.sender.timeout(10_000, IDLE_TIMEOUT)?;

        let msg = msg.as_text()?;
        if msg == "null" {
            // Fast path for ping.
            return self.sender.send(Message::text("0"));
        }

        match serde_json::from_str(msg) {
            Ok(SocketOut::Ping { .. }) => {
                self.sender.send(Message::text("0"))
            }
            Ok(SocketOut::Notified) => {
                if let Some(ref uid) = self.uid {
                    log::debug!("notified: {}", uid);
                    self.app.publish(LilaIn::Notified { user: uid.to_owned() });
                }
                Ok(())
            }
            Ok(SocketOut::StartWatching { d }) => {
                log::debug!("start watching: {}", d);
                self.app.publish(LilaIn::Watch { game: d });
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
        //let timeout = dbg!(timeout);
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
}

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

            let _: () = redis.set("connections", 0).expect("reset connections");

            loop {
                match redis_recv.recv().expect("redis recv") {
                    LilaIn::Inc => redis.incr("connections", 1).expect("incr connections"),
                    LilaIn::Dec => redis.incr("connections", -1).expect("decr connections"),
                    msg => {
                        let msg = serde_json::to_string(&msg).expect("serialize");
                        let ret: u32 = redis.publish("site-in", msg).expect("publish site-in");
                        if ret == 0 {
                            log::error!("lila missed as message");
                        }
                    },
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
            incoming.subscribe("site-out").expect("subscribe site-out");

            loop {
                let redis_msg = incoming.get_message().expect("get message");
                let payload: String = redis_msg.get_payload().expect("get payload");
                log::debug!("site-out: {}", payload);
                let msg: LilaOut = serde_json::from_str(&payload).expect("lila out");
                app.received(msg);
            }
        });

        let mut settings = ws::Settings::default();
        settings.max_connections = 40_000;
        settings.tcp_nodelay = true;

        let server = ws::Builder::new()
            .with_settings(settings)
            .build(move |sender| {
                Socket {
                    app,
                    sender,
                    uid: None,
                    watching: HashSet::new(),
                    idle_timeout: None
                }
            })
            .expect("valid settings");

        app.broadcaster.set(server.broadcaster()).expect("set broadcaster");

        server
            .listen("127.0.0.1:9664")
            .expect("ws listen");
    }).expect("scope");
}
