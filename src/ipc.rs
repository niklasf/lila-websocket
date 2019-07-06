use std::fmt;

use smallvec::SmallVec;

use crate::model::{Flag, GameId, UserId, InvalidUserId};

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
    MoveLatency(u32),
}

impl<'a> LilaOut<'a> {
    pub fn parse(s: &'a str) -> Result<LilaOut<'a>, IpcError> {
        let mut tag_and_args = s.splitn(2, ' ');
        Ok(match (tag_and_args.next(), tag_and_args.next()) {
            (Some("move"), Some(args)) => {
                let mut args = args.splitn(3, ' ');
                LilaOut::Move {
                    game: args.next().ok_or(IpcError)?.parse().map_err(|_| IpcError)?,
                    last_uci: args.next().ok_or(IpcError)?,
                    fen: args.next().ok_or(IpcError)?,
                }
            },
            (Some("tell/user"), Some(args)) | (Some("tell/users"), Some(args)) => {
                let mut args = args.splitn(2, ' ');
                let maybe_users: Result<_, InvalidUserId> = args.next().ok_or(IpcError)?.split(',').map(|u| UserId::new(u)).collect();
                LilaOut::TellUsers {
                    users: maybe_users.map_err(|_| IpcError)?,
                    payload: args.next().ok_or(IpcError)?,
                }
            },
            (Some("tell/all"), Some(payload)) => {
                LilaOut::TellAll { payload }
            },
            (Some("tell/flag"), Some(args)) => {
                let mut args = args.splitn(2, ' ');
                LilaOut::TellFlag {
                    flag: args.next().ok_or(IpcError)?.parse().map_err(|_| IpcError)?,
                    payload: args.next().ok_or(IpcError)?,
                }
            },
            (Some("mlat"), Some(value)) => {
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
    Notified(&'a UserId),
    Watch(&'a GameId),
    Connections(u32),
}

impl<'a> fmt::Display for LilaIn<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LilaIn::Connect(uid) => write!(f, "connect {}", uid),
            LilaIn::Disconnect(uid) => write!(f, "disconnect {}", uid),
            LilaIn::Notified(uid) => write!(f, "notified {}", uid),
            LilaIn::Watch(game) => write!(f, "watch {}", game),
            LilaIn::Connections(n) => write!(f, "connections {}", n),
        }
    }
}
