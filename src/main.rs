use cookie::Cookie;
use websocket::r#async::Server;

use websocket::server::InvalidConnection;
use websocket::header;
use websocket::header::{Headers};

use mongodb::ThreadedClient;

use futures::{Future, Sink, Stream, future, stream};

use std::str;
use std::borrow::Cow;

#[derive(Debug)]
struct Sid(String);

fn session_id(headers: &Headers) -> Option<Sid> {
    let headers: &Vec<String> = headers.get::<header::Cookie>()?;

    for header in headers {
        let cookie = Cookie::parse(header).ok()?;
        let (name, sid) = cookie.name_value();
        if name == "lila2" {
            return Some(Sid(sid.to_owned()));
        }
    }

    None
}

fn user_id(sid: &Sid) -> Option<String> {
    let client = mongodb::Client::connect("127.0.0.1", 27017).unwrap();

    None
}

fn main() {
    let mut runtime = tokio::runtime::Builder::new().build().unwrap();
    let executor = runtime.executor();

    let server = Server::bind("127.0.0.1:9664", &tokio::reactor::Handle::default()).unwrap();

    let f = server
        .incoming()
        .inspect_err(|err| println!("Error: ..."))
        .then(|r| future::ok(stream::iter_ok::<_, ()>(r)))
        .flatten()
        .for_each(move |(upgrade, addr)| {
            let sid = dbg!(session_id(&upgrade.request.headers));

            if let Some(sid) = sid {
                dbg!(user_id(&sid));
            }

            let f = upgrade.accept().and_then(|(s, _)| {
                let (sink, stream) = s.split();
                stream
                    .map(|v| {
                        dbg!(v)
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
