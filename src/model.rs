use arrayvec::ArrayString;

use std::str::FromStr;
use std::fmt;

use serde::{Deserialize, Serialize, Serializer, Deserializer};

/// An 8 character game id.
#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub struct GameId(ArrayString<[u8; 8]>);

#[derive(Debug)]
pub struct InvalidGameId;

impl GameId {
    pub fn new(inner: ArrayString<[u8; 8]>) -> Result<GameId, InvalidGameId> {
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

impl fmt::Display for GameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
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

/// Username, normalized to lowercase.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct UserId(unicase::Ascii<String>);

#[derive(Debug)]
pub struct InvalidUserId;

impl UserId {
    pub fn new(inner: &str) -> Result<UserId, InvalidUserId> {
        if !inner.is_empty() && inner.len() <= 30 &&
           inner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            Ok(UserId(inner.parse().map_err(|_| InvalidUserId)?))
        } else {
            Err(InvalidUserId)
        }
    }
}

impl Serialize for UserId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for UserId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let inner = String::deserialize(deserializer)?;
        UserId::new(&inner).map_err(|_| serde::de::Error::custom("invalid user id"))
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
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
