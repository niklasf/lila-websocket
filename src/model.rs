use serde::Deserialize;

/// Channels for server sent updates.
#[derive(Deserialize, Debug, Copy, Clone)]
pub enum Flag {
    #[serde(rename = "simul")]
    Simul = 0,
    #[serde(rename = "tournament")]
    Tournament = 1,
}
