use crate::model::Flag;

#[derive(Debug)]
struct IpcError;

/// Messages we receive from lila.
#[derive(Debug)]
enum LilaOut<'a> {
    Move {
        game: &'a str,
        last_uci: &'a str,
        fen: &'a str,
    },
    Tell {
        users: Vec<&'a str>,
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
    fn parse(s: &'a str) -> Result<LilaOut<'a>, IpcError> {
        let mut tag_and_args = s.splitn(2, ' ');
        Ok(match (tag_and_args.next(), tag_and_args.next()) {
            (Some("move"), Some(args)) => {
                let mut args = args.splitn(3, ' ');
                LilaOut::Move {
                    game: args.next().ok_or(IpcError)?,
                    last_uci: args.next().ok_or(IpcError)?,
                    fen: args.next().ok_or(IpcError)?,
                }
            },
            (Some("tell"), Some(args)) => {
                let mut args = args.splitn(2, ' ');
                LilaOut::Tell {
                    users: args.next().ok_or(IpcError)?.split(',').collect(),
                    payload: args.next().ok_or(IpcError)?,
                }
            },
            _ => return Err(IpcError),
        })
    }
}
