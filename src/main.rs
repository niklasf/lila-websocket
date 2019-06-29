use websocket::r#async::Server;

use websocket::server::InvalidConnection;

use futures::{Future, Stream, future, stream};

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
            dbg!(addr);
            upgrade.accept().and_then(|(s, _)| {
                future::ok(())
            });
            Ok(())
        });

    runtime.block_on(f).unwrap();
}
