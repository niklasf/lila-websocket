use arrayvec::ArrayString;

use std::str::FromStr;
use std::fmt;

use serde::{Deserialize, Serialize, Serializer, Deserializer};

/// An 8 character game id.
#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub struct GameId(ArrayString<[u8; 8]>);

#[derive(Debug)]
pub struct InvalidGameId;

impl fmt::Display for InvalidGameId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("invalid game id")
    }
}

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
pub struct UserId(String);

#[derive(Debug)]
pub struct InvalidUserId;

impl UserId {
    pub fn new(inner: &str) -> Result<UserId, InvalidUserId> {
        if !inner.is_empty() && inner.len() <= 30 &&
           inner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            Ok(UserId(inner.to_lowercase()))
        } else {
            Err(InvalidUserId)
        }
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
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

/// Uniquely identifies a page view. The sri stays the same across reconnects
/// on the same page, but changes when navigating to a different page.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Sri(ArrayString<[u8; 12]>);

#[derive(Debug)]
pub struct InvalidSri;

impl Sri {
    pub fn new(inner: ArrayString<[u8; 12]>) -> Result<Sri, InvalidSri> {
        if inner.chars().all(|c| c != ' ') {
            Ok(Sri(inner))
        } else {
            Err(InvalidSri)
        }
    }
}

impl<'de> Deserialize<'de> for Sri {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let inner = ArrayString::deserialize(deserializer)?;
        Sri::new(inner).map_err(|_| serde::de::Error::custom("invalid sri"))
    }
}

impl FromStr for Sri {
    type Err = InvalidSri;

    fn from_str(s: &str) -> Result<Sri, InvalidSri> {
        Sri::new(ArrayString::from(s).map_err(|_| InvalidSri)?)
    }
}

impl fmt::Display for Sri {
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

#[derive(Debug)]
pub struct UnknownFlag;

impl FromStr for Flag {
    type Err = UnknownFlag;

    fn from_str(s: &str) -> Result<Flag, UnknownFlag> {
        Ok(match s {
            "tournament" => Flag::Tournament,
            "simul" => Flag::Simul,
            _ => return Err(UnknownFlag),
        })
    }
}

/// The type of socket
#[derive(Deserialize, Debug, Copy, Clone)]
pub enum Endpoint {
    #[serde(rename = "site")]
    Site = 0,
    #[serde(rename = "lobby")]
    Lobby = 1,
}

#[derive(Debug)]
pub struct UnknownEndpoint;

impl FromStr for Endpoint {
    type Err = UnknownEndpoint;

    fn from_str(s: &str) -> Result<Endpoint, UnknownEndpoint> {
        Ok(match s {
            "/socket/v4" => Endpoint::Site,
            "/lobby/socket/v4" => Endpoint::Lobby,
            _ => return Err(UnknownEndpoint),
        })
    }
}
