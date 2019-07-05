use arrayvec::ArrayString;

use std::str::FromStr;

use serde::{Deserialize, Serialize, Serializer, Deserializer};

/// An 8 charatcer game id.
#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub struct GameId(ArrayString<[u8; 8]>);

#[derive(Debug)]
pub struct InvalidGameId;

impl GameId {
    fn new(inner: ArrayString<[u8; 8]>) -> Result<GameId, InvalidGameId> {
        if inner.chars().all(|c| c.is_ascii_alphanumeric()) && inner.len() == 8 {
            Ok(GameId(inner))
        } else {
            Err(InvalidGameId)
        }
    }
}

impl Serialize for GameId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GameId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let inner = ArrayString::deserialize(deserializer)?;
        GameId::new(inner).map_err(|_| serde::de::Error::custom("invalid game id"))
    }
}

impl FromStr for GameId {
    type Err = InvalidGameId;

    fn from_str(s: &str) -> Result<GameId, InvalidGameId> {
        GameId::new(ArrayString::from(s).map_err(|_| InvalidGameId)?)
    }
}

/// Channels for server sent updates.
#[derive(Deserialize, Debug, Copy, Clone)]
pub enum Flag {
    #[serde(rename = "simul")]
    Simul = 0,
    #[serde(rename = "tournament")]
    Tournament = 1,
}
