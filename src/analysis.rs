use serde::{Deserialize, Serialize};

use shakmaty::Square;
use shakmaty::fen::{Fen, FenOpts};

use crate::opening_db::{Opening, FULL_OPENING_DB};

fn lookup_opening(mut fen: Fen) -> Option<&'static Opening> {
    fen.pockets = None;
    fen.remaining_checks = None;
    FULL_OPENING_DB.get(FenOpts::new().epd(&fen).as_str())
}

fn piotr(sq: Square) -> u8 {
    if sq < Square::C4 {
        b'a' + u8::from(sq)
    } else if sq < Square::E7 {
        b'A' + (sq - Square::C4) as u8
    } else if sq < Square::G8 {
        b'0' + (sq - Square::E7) as u8
    } else if sq == Square::G8 {
        b'!'
    } else {
        b'?'
    }
}

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
            self.fen.parse().ok()
                .and_then(lookup_opening)
                .map(|opening| OpeningResponse {
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
        let dests: String = "".to_owned(); // TODO

        Ok(DestsResponse {
            path: self.path,
            opening: lookup_opening(fen),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piotr() {
        assert_eq!(piotr(Square::A1), b'a');
        assert_eq!(piotr(Square::B4), b'z');
        assert_eq!(piotr(Square::C4), b'A');
        assert_eq!(piotr(Square::D7), b'Z');
        assert_eq!(piotr(Square::E7), b'0');
        assert_eq!(piotr(Square::F8), b'9');
        assert_eq!(piotr(Square::G8), b'!');
        assert_eq!(piotr(Square::H8), b'?');
    }
}
