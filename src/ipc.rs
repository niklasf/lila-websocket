use std::fmt;

use smallvec::SmallVec;
use std::collections::HashMap;

use crate::model::{Flag, GameId, Sri, UserId, InvalidUserId};

#[derive(Debug)]
pub struct IpcError;

/// Messages we receive from lila.
#[derive(Debug)]
pub enum LilaOut<'a> {
    Move {
        game: GameId,
        last_uci: &'a str,
        fen: &'a str,
    },
    TellUsers {
        users: SmallVec<[UserId; 1]>,
        payload: &'a str,
    },
    TellAll {
        payload: &'a str,
    },
    TellFlag {
        flag: Flag,
        payload: &'a str,
    },
    TellSri {
        sri: Sri,
        payload: &'a str,
    },
    DisconnectUser {
        uid: UserId,
    },
    MoveLatency(u32),
}

impl<'a> LilaOut<'a> {
    pub fn parse(s: &'a str) -> Result<LilaOut<'a>, IpcError> {
        let mut tag_and_args = s.splitn(2, ' ');
        Ok(match (tag_and_args.next().unwrap(), tag_and_args.next()) {
            ("move", Some(args)) => {
                let mut args = args.splitn(3, ' ');
                LilaOut::Move {
                    game: args.next().unwrap().parse().map_err(|_| IpcError)?,
                    last_uci: args.next().ok_or(IpcError)?,
                    fen: args.next().ok_or(IpcError)?,
                }
            },
            ("tell/user", Some(args)) | ("tell/users", Some(args)) => {
                let mut args = args.splitn(2, ' ');
                let maybe_users: Result<_, InvalidUserId> = args.next().unwrap().split(',').map(UserId::new).collect();
                LilaOut::TellUsers {
                    users: maybe_users.map_err(|_| IpcError)?,
                    payload: args.next().ok_or(IpcError)?,
                }
            },
            ("tell/all", Some(payload)) => {
                LilaOut::TellAll { payload }
            },
            ("tell/flag", Some(args)) => {
                let mut args = args.splitn(2, ' ');
                LilaOut::TellFlag {
                    flag: args.next().ok_or(IpcError)?.parse().map_err(|_| IpcError)?,
                    payload: args.next().ok_or(IpcError)?,
                }
            },
            ("tell/sri", Some(args)) => {
                let mut args = args.splitn(2, ' ');
                LilaOut::TellSri {
                    sri: args.next().unwrap().parse().map_err(|_| IpcError)?,
                    payload: args.next().ok_or(IpcError)?,
                }
            },
            ("disconnect/user", Some(uid)) => {
                LilaOut::DisconnectUser {
                    uid: UserId::new(uid).map_err(|_| IpcError)?,
                }
            }
            ("mlat", Some(value)) => {
                LilaOut::MoveLatency(value.parse().map_err(|_| IpcError)?)
            },
            _ => return Err(IpcError),
        })
    }
}

/// Messages we send to lila.
#[derive(Debug)]
pub enum LilaIn<'a> {
    Connect(&'a UserId),
    Disconnect(&'a UserId),
    DisconnectAll,
    Notified(&'a UserId),
    Watch(&'a GameId),
    Unwatch(&'a GameId),
    Connections(u32),
    Lags(&'a HashMap::<UserId, u32>),
    Friends(&'a UserId),
    TellSri(&'a Sri, Option<&'a UserId>, &'a str),
}

impl<'a> fmt::Display for LilaIn<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LilaIn::Connect(uid) => write!(f, "connect {}", uid),
            LilaIn::Disconnect(uid) => write!(f, "disconnect {}", uid),
            LilaIn::DisconnectAll => write!(f, "disconnect/all"),
            LilaIn::Notified(uid) => write!(f, "notified {}", uid),
            LilaIn::Watch(game) => write!(f, "watch {}", game),
            LilaIn::Unwatch(game) => write!(f, "unwatch {}", game),
            LilaIn::Connections(n) => write!(f, "connections {}", n),
            LilaIn::Lags(lags) => {
                write!(f, "lags ")?;
                for (uid, lag) in lags.iter() { 
                    write!(f, "{}:{},", uid, lag)?;
                }
                Ok(())
            }
            LilaIn::Friends(uid) => write!(f, "friends {}", uid),
            LilaIn::TellSri(sri, uid, payload) =>
                write!(f, "tell/sri {} {} {}", sri, uid.map_or("-", |u| u.as_str()), payload),
        }
    }
}
