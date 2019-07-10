#!/bin/sh -e
cargo build --release
ssh root@greco.lichess.ovh mv /usr/local/bin/lila-websocket /usr/local/bin/lila-websocket.bak
scp ./target/release/lila-websocket root@greco.lichess.ovh:/usr/local/bin/lila-websocket
ssh root@greco.lichess.ovh systemctl restart lila-websocket
