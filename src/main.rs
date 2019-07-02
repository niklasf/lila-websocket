// -> site-in (to lila)
// <- site-out (from lila)
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

use cookie::Cookie;

use mio_extras::timer::Timeout;

use serde::{Serialize, Deserialize};

use ws::{Handshake, Handler, Frame, Sender, Message, CloseCode};
use ws::util::Token;

use redis::RedisResult;
use redis::Commands as _;

use std::collections::HashMap;
use std::str;
use std::sync::{Arc, Mutex};

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

    mongodb::Client::connect("127.0.0.1", 27017)
        .expect("mongodb connection")
        .db("lichess")
        .collection("security")
        .find_one(Some(query), None)
        .expect("query by sid")
        .and_then(|doc| doc.get_str("user").map(|s| s.to_owned()).ok())
}

struct App {
    by_user: Mutex<HashMap::<String, Vec<Sender>>>,
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
            redis: Mutex::new(redis),
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
            idle_timeout: None
        }
    }).expect("ws listen");
}

#[derive(Serialize)]
struct MsgConnect {
    path: &'static str,
    data: MsgConnectData,
}

#[derive(Serialize)]
struct MsgConnectData {
    user: String,
}

fn publish_connect(con: &mut redis::Connection, uid: String) -> RedisResult<u32> {
    con.publish("site-in", serde_json::to_string(&MsgConnect {
        path: "/connect",
        data: MsgConnectData {
            user: uid
        }
    }).expect("serialize connect"))
}

fn publish_disconnect(con: &mut redis::Connection, uid: String) -> RedisResult<u32> {
    con.publish("site-in", serde_json::to_string(&MsgConnect {
        path: "/disconnect",
        data: MsgConnectData {
            user: uid
        }
    }).expect("serialize connect"))
}

struct DefaultHandler;
impl Handler for DefaultHandler { }

struct Server {
    app: Arc<App>,
    sender: Sender,
    uid: Option<String>,
    idle_timeout: Option<Timeout>,
}

#[derive(Deserialize)]
#[serde(tag = "t")]
enum ClientMessage {
    #[serde(rename = "p")]
    Ping { l: u32 }
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
                    let mut redis = self.app.redis.lock().expect("lock redis");
                    let n = publish_connect(&mut redis, uid.to_owned()).expect("publish connect");
                    println!("connected: {} (ack: {})", uid, n);
                    vec![self.sender.clone()]
                });
        }

        self.sender.timeout(10_000, IDLE_TIMEOUT)
    }

    fn on_close(&mut self, _: CloseCode, _: &str) {
        if let Some(uid) = self.uid.take() {
            let mut by_user = self.app.by_user.lock().expect("lock by_user for close");
            let entry = by_user.get_mut(&uid).expect("uid in map");
            let len_before = entry.len();
            entry.retain(|s| s.token() != self.sender.token());
            assert_eq!(entry.len() + 1, len_before);
            if entry.is_empty() {
                by_user.remove(&uid);
                let mut redis = self.app.redis.lock().expect("lock redis");
                let n = publish_disconnect(&mut redis, uid.clone()).expect("publish disconnect");
                println!("disconnected: {} (ack: {})", uid, n);
            }
        }
    }

    fn on_message(&mut self, msg: Message) -> ws::Result<()> {
        if msg.as_text()? == "null" { // ping
            self.sender.send(Message::text("0"))
        } else {
            match serde_json::from_str(msg.as_text()?) {
                Ok(ClientMessage::Ping { .. }) => {
                    self.sender.send(Message::text("0"))
                }
                Err(err) => {
                    println!("protocol violation: {:?}", err);
                    self.sender.close(CloseCode::Protocol)
                }
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
        println!("on frame");
        self.sender.timeout(10_000, IDLE_TIMEOUT)?;
        DefaultHandler.on_frame(frame)
    }
}

/* struct Connection {
    user_id: Option<String>,
//  sink: Sink,
}

#[derive(Serialize)]
struct Msg {
    t: &'static str,
    v: String,
}

impl Msg {
    fn connect(uid: String) -> Msg {
        Msg {
            t: "connect",
            v: uid,
        }
    }
}

fn main() {
    let mut runtime = tokio::runtime::Builder::new().build().unwrap();
    let executor = runtime.executor();

    let redis = Arc::new(runtime.block_on(
        redis_async::client::paired::paired_connect(&"127.0.0.1:6379".parse().unwrap())
    ).unwrap());

    let f = pubsub::pubsub_connect(&"127.0.0.1:6379".parse().unwrap())
        .and_then(|redis| redis.subscribe("res"))
        .map_err(|e| panic!("redis: {:?}", e))
        .and_then(|chan| {
            chan.for_each(|msg| {
                dbg!(msg);
                Ok(())
            })
        });

    executor.spawn(f);

    let server = Server::bind("127.0.0.1:9664", &tokio::reactor::Handle::default()).unwrap();

    let f = server
        .incoming()
        .inspect_err(|e| println!("Error: {:?}", e.error))
        .then(|r| future::ok(stream::iter_ok::<_, ()>(r)))
        .flatten()
        .for_each(move |(upgrade, addr)| {
            let sid = match lila2(&upgrade.request.headers).and_then(|l| session_id(&l)) {
                Some(sid) => sid,
                None => {
                    upgrade.reject(); // TODO: await
                    return Ok(());
                }
            };

            let uid = user_id(&sid);
            let redis_inner = redis.clone();

            let f = upgrade.accept().and_then(move |(s, _)| {
                let (mut sink, stream) = s.split();

                if let Some(uid) = uid {
                    redis_inner.send_and_forget(resp_array![
                        "PUBLISH",
                        "chan",
                        serde_json::to_string(&Msg::connect(uid)).unwrap()
                    ]);
                }

                //sink.start_send(OwnedMessage::Text("foo".to_owned())); // TODO: await

                stream
                    .take_while(|m| Ok(!m.is_close()))
                    .filter_map(|v| {
                        dbg!(v);
                        Some(OwnedMessage::Text("0".to_owned()))
                    })
                    .forward(sink)
            });


            executor.spawn(
                f.map_err(move |e| println!("{}: '{:?}'", addr, e))
                     .map(move |_| println!("{} closed.", addr)),
            );

            Ok(())
        });

    runtime.block_on(f).unwrap();
} */
