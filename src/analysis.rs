use serde::{Deserialize, Serialize};

use shakmaty::fen::Fen;

use crate::opening_db::{Opening, FULL_OPENING_DB};

#[derive(Deserialize, Copy, Clone)]
enum VariantKey {
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "fromPosition")]
    FromPosition,
    #[serde(rename = "chess960")]
    Chess960,
    #[serde(rename = "antichess")]
    Antichess,
    #[serde(rename = "kingOfTheHill")]
    KingOfTheHill,
    #[serde(rename = "threeCheck")]
    ThreeCheck,
    #[serde(rename = "atomic")]
    Atomic,
    #[serde(rename = "horde")]
    Horde,
    #[serde(rename = "racingKings")]
    RacingKings,
    #[serde(rename = "crazyhouse")]
    Crazyhouse,
}

#[derive(Copy, Clone)]
enum EffectiveVariantKey {
    Standard,
    Antichess,
    KingOfTheHill,
    ThreeCheck,
    Atomic,
    Horde,
    RacingKings,
    Crazyhouse,
}

impl EffectiveVariantKey {
    fn is_opening_sensible(self) -> bool {
        match self {
            EffectiveVariantKey::Standard |
            EffectiveVariantKey::Crazyhouse |
            EffectiveVariantKey::ThreeCheck |
            EffectiveVariantKey::KingOfTheHill => true,
            _ => false,
        }
    }
}

impl From<VariantKey> for EffectiveVariantKey {
    fn from(variant: VariantKey) -> EffectiveVariantKey {
        match variant {
            VariantKey::Standard | VariantKey::FromPosition | VariantKey::Chess960 =>
                EffectiveVariantKey::Standard,
            VariantKey::Antichess => EffectiveVariantKey::Antichess,
            VariantKey::KingOfTheHill => EffectiveVariantKey::KingOfTheHill,
            VariantKey::ThreeCheck => EffectiveVariantKey::ThreeCheck,
            VariantKey::Atomic => EffectiveVariantKey::Atomic,
            VariantKey::Horde => EffectiveVariantKey::Horde,
            VariantKey::RacingKings => EffectiveVariantKey::RacingKings,
            VariantKey::Crazyhouse => EffectiveVariantKey::Crazyhouse,
        }
    }
}

#[derive(Deserialize)]
pub struct GetOpening {
    variant: Option<VariantKey>,
    path: String,
    fen: String,
}

impl GetOpening {
    pub fn respond(self) -> Option<OpeningResponse> {
        let variant = EffectiveVariantKey::from(self.variant.unwrap_or(VariantKey::Standard));
        if variant.is_opening_sensible() {
            let epd: String = self.fen.split(' ').take(4).collect();
            FULL_OPENING_DB.get(epd.as_str()).map(|opening| OpeningResponse {
                path: self.path,
                opening
            })
        } else {
            None
        }
    }
}

#[derive(Serialize)]
pub struct OpeningResponse {
    path: String,
    opening: &'static Opening,
}

#[derive(Deserialize)]
pub struct GetDests {
    variant: Option<VariantKey>,
    fen: String,
    path: String,
    #[serde(rename = "ch")]
    chapter_id: Option<String>,
}

impl GetDests {
    pub fn respond(self) -> Result<DestsResponse, DestsFailure> {
        let fen: Fen = self.fen.parse().map_err(|_| DestsFailure)?;
        let epd: String = "".to_owned(); // TODO
        let dests: String = "".to_owned(); // TODO

        Ok(DestsResponse {
            path: self.path,
            opening: FULL_OPENING_DB.get(epd.as_str()),
            chapter_id: self.chapter_id,
            dests,
        })
    }
}

#[derive(Serialize)]
pub struct DestsResponse {
    path: String,
    dests: String,
    #[serde(flatten)]
    opening: Option<&'static Opening>,
    #[serde(rename = "ch", flatten)]
    chapter_id: Option<String>,
}

#[derive(Debug)]
pub struct DestsFailure;
