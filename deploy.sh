#!/bin/sh -e
cargo build --release
ssh root@greco.lichess.ovh rm /usr/local/bin/lila-websocket
scp ./target/release/lila-websocket root@greco.lichess.ovh:/usr/local/bin/lila-websocket
ssh root@greco.lichess.ovh systemctl restart lila-websocket
