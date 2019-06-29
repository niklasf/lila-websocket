use cookie::Cookie;
use websocket::r#async::Server;

use websocket::server::InvalidConnection;
use websocket::header;
use websocket::header::{Headers};

use futures::{Future, Stream, future, stream};

use std::str;
use std::borrow::Cow;

fn session_id(headers: &Headers) -> Option<String> {
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
            dbg!(session_id(&upgrade.request.headers));
            dbg!(addr);
            dbg!(&upgrade.request);
            upgrade.accept().and_then(|(s, _)| {
                future::ok(())
            });
            Ok(())
        });

    runtime.block_on(f).unwrap();
}
