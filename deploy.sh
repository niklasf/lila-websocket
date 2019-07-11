#!/bin/sh -e
cargo +stable build --release
ssh "root@$1.lichess.ovh" mv /usr/local/bin/lila-websocket /usr/local/bin/lila-websocket.bak || (echo "first deploy on this server? comment out this line" && false)
scp ./target/release/lila-websocket "root@$1.lichess.ovh":/usr/local/bin/lila-websocket
ssh "root@$1.lichess.ovh" systemctl restart lila-websocket
