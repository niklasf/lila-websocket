use cookie::Cookie;
use websocket::r#async::Server;

use websocket::server::InvalidConnection;
use websocket::header;
use websocket::header::{Headers};
use websocket::message::OwnedMessage;

use mongodb::ThreadedClient as _;
use mongodb::db::ThreadedDatabase as _;

use redis_async::client::pubsub;

use futures::{Future, Sink, Stream, future, stream};

use serde::Deserialize;

use std::str;

#[derive(Debug, Deserialize)]
struct SessionCookie {
    #[allow(non_snake_case)]
    sessionId: String,
}

fn lila2(headers: &Headers) -> Option<String> {
    let headers: &Vec<String> = headers.get::<header::Cookie>()?;

    for header in headers {
        let cookie = Cookie::parse(header).ok()?;
        let (name, sid) = cookie.name_value();
        if name == "lila2" {
            return Some(sid.to_owned());
        }
    }

    None
}

fn session_id(lila2: &str) -> Option<SessionCookie> {
    serde_urlencoded::from_str(lila2).ok()
}

fn user_id(cookie: &SessionCookie) -> Option<String> {
    let mut query = mongodb::Document::new();
    query.insert("_id", &cookie.sessionId);

    mongodb::Client::connect("127.0.0.1", 27017)
        .expect("mongodb connection")
        .db("lichess")
        .collection("security")
        .find_one(Some(query), None)
        .expect("query by sid")
        .and_then(|doc| doc.get_str("user").map(|s| s.to_owned()).ok())
}

fn main() {
    let mut runtime = tokio::runtime::Builder::new().build().unwrap();
    let executor = runtime.executor();

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

            dbg!(&sid);
            dbg!(user_id(&sid));

            let f = upgrade.accept().and_then(|(s, _)| {
                let (mut sink, stream) = s.split();
                sink.start_send(OwnedMessage::Text("foo".to_owned())); // TODO: await

                stream
                    .map(|v| {
                        dbg!(v);
                        OwnedMessage::Text("0".to_owned())
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
}
