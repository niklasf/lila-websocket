use std::mem;

use serde::{Deserialize, Serialize};

use arrayvec::ArrayString;

use shakmaty::{Square, PositionError, Position, MoveList, Role, IllegalMoveError, Move};
use shakmaty::variants::{Chess, Giveaway, KingOfTheHill, ThreeCheck, Atomic, Horde, RacingKings, Crazyhouse};
use shakmaty::fen::{Fen, FenOpts};
use shakmaty::san::SanPlus;
use shakmaty::uci::Uci;

use crate::opening_db::{Opening, FULL_OPENING_DB};
use crate::util;

fn fen_from_setup(setup: &dyn Position) -> Fen {
    Fen {
        board: setup.board().clone(),
        pockets: setup.pockets().cloned(),
        turn: setup.turn(),
        castling_rights: setup.castling_rights(),
        ep_square: setup.ep_square(),
        remaining_checks: setup.remaining_checks().cloned(),
        halfmoves: setup.halfmoves(),
        fullmoves: setup.fullmoves(),
    }
}

fn lookup_opening(mut fen: Fen) -> Option<&'static Opening> {
    fen.pockets = None;
    fen.remaining_checks = None;
    FULL_OPENING_DB.get(FenOpts::new().epd(&fen).as_str())
}

fn uci_char_pair(from: Square, to: Square) -> ArrayString<[u8; 2]> {
    let mut r = ArrayString::new();
    r.push(piotr(from));
    r.push(piotr(to));
    r
}

fn piotr(sq: Square) -> char {
    if sq < Square::C4 {
        (b'a' + u8::from(sq)) as char
    } else if sq < Square::E7 {
        (b'A' + (sq - Square::C4) as u8) as char
    } else if sq < Square::G8 {
        (b'0' + (sq - Square::E7) as u8) as char
    } else if sq == Square::G8 {
        '!'
    } else {
        '?'
    }
}

fn dests(pos: &dyn Position) -> String {
    let mut legals = MoveList::new();
    pos.legal_moves(&mut legals);

    let mut dests = String::with_capacity(80);
    let mut first = true;
    for from_sq in pos.us() {
        let mut from_here = legals.iter().filter(|m| m.from() == Some(from_sq)).peekable();
        if from_here.peek().is_some() {
            if mem::replace(&mut first, false) {
                dests.push(' ');
            }
            dests.push(piotr(from_sq));
            for m in from_here {
                dests.push(piotr(m.to()));
            }
        }
    }

    dests
}

#[derive(Deserialize)]
enum PromotableRole {
    #[serde(rename = "knight")]
    Knight,
    #[serde(rename = "bishop")]
    Bishop,
    #[serde(rename = "rook")]
    Rook,
    #[serde(rename = "queen")]
    Queen,
    #[serde(rename = "king")]
    King,
}

impl From<PromotableRole> for Role {
    fn from(r: PromotableRole) -> Role {
        match r {
            PromotableRole::Knight => Role::Knight,
            PromotableRole::Bishop => Role::Bishop,
            PromotableRole::Rook => Role::Rook,
            PromotableRole::Queen => Role::Queen,
            PromotableRole::King => Role::King,
        }
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

    fn position(self, fen: &Fen) -> Result<VariantPosition, PositionError> {
        match self {
            EffectiveVariantKey::Standard => fen.position().map(VariantPosition::Standard),
            EffectiveVariantKey::Antichess => fen.position().map(VariantPosition::Antichess),
            EffectiveVariantKey::KingOfTheHill => fen.position().map(VariantPosition::KingOfTheHill),
            EffectiveVariantKey::ThreeCheck => fen.position().map(VariantPosition::ThreeCheck),
            EffectiveVariantKey::Atomic => fen.position().map(VariantPosition::Atomic),
            EffectiveVariantKey::Horde => fen.position().map(VariantPosition::Horde),
            EffectiveVariantKey::RacingKings => fen.position().map(VariantPosition::RacingKings),
            EffectiveVariantKey::Crazyhouse => fen.position().map(VariantPosition::Crazyhouse),
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

#[derive(Clone)]
enum VariantPosition {
    Standard(Chess),
    Antichess(Giveaway),
    KingOfTheHill(KingOfTheHill),
    ThreeCheck(ThreeCheck),
    Atomic(Atomic),
    Horde(Horde),
    RacingKings(RacingKings),
    Crazyhouse(Crazyhouse),
}

impl VariantPosition {
    fn borrow(&self) -> &dyn Position {
        match *self {
            VariantPosition::Standard(ref pos) => pos,
            VariantPosition::Antichess(ref pos) => pos,
            VariantPosition::KingOfTheHill(ref pos) => pos,
            VariantPosition::ThreeCheck(ref pos) => pos,
            VariantPosition::Atomic(ref pos) => pos,
            VariantPosition::Horde(ref pos) => pos,
            VariantPosition::RacingKings(ref pos) => pos,
            VariantPosition::Crazyhouse(ref pos) => pos,
        }
    }

    fn borrow_mut(&mut self) -> &mut dyn Position {
        match *self {
            VariantPosition::Standard(ref mut pos) => pos,
            VariantPosition::Antichess(ref mut pos) => pos,
            VariantPosition::KingOfTheHill(ref mut pos) => pos,
            VariantPosition::ThreeCheck(ref mut pos) => pos,
            VariantPosition::Atomic(ref mut pos) => pos,
            VariantPosition::Horde(ref mut pos) => pos,
            VariantPosition::RacingKings(ref mut pos) => pos,
            VariantPosition::Crazyhouse(ref mut pos) => pos,
        }
    }

    fn fen(&self) -> String {
        match *self {
            VariantPosition::Standard(ref pos) => FenOpts::default().fen(pos),
            VariantPosition::Antichess(ref pos) => FenOpts::default().fen(pos),
            VariantPosition::KingOfTheHill(ref pos) => FenOpts::default().fen(pos),
            VariantPosition::ThreeCheck(ref pos) => FenOpts::default().fen(pos),
            VariantPosition::Atomic(ref pos) => FenOpts::default().fen(pos),
            VariantPosition::Horde(ref pos) => FenOpts::default().fen(pos),
            VariantPosition::RacingKings(ref pos) => FenOpts::default().fen(pos),
            VariantPosition::Crazyhouse(ref pos) => FenOpts::default().fen(pos),
        }
    }

    fn uci_to_move(&self, uci: &Uci) -> Result<Move, IllegalMoveError> {
        match *self {
            VariantPosition::Standard(ref pos) => uci.to_move(pos),
            VariantPosition::Antichess(ref pos) => uci.to_move(pos),
            VariantPosition::KingOfTheHill(ref pos) => uci.to_move(pos),
            VariantPosition::ThreeCheck(ref pos) => uci.to_move(pos),
            VariantPosition::Atomic(ref pos) => uci.to_move(pos),
            VariantPosition::Horde(ref pos) => uci.to_move(pos),
            VariantPosition::RacingKings(ref pos) => uci.to_move(pos),
            VariantPosition::Crazyhouse(ref pos) => uci.to_move(pos),
        }
    }

    fn san_plus(self, m: &Move) -> SanPlus {
        match self {
            VariantPosition::Standard(pos) => SanPlus::from_move(pos, m),
            VariantPosition::Antichess(pos) => SanPlus::from_move(pos, m),
            VariantPosition::KingOfTheHill(pos) => SanPlus::from_move(pos, m),
            VariantPosition::ThreeCheck(pos) => SanPlus::from_move(pos, m),
            VariantPosition::Atomic(pos) => SanPlus::from_move(pos, m),
            VariantPosition::Horde(pos) => SanPlus::from_move(pos, m),
            VariantPosition::RacingKings(pos) => SanPlus::from_move(pos, m),
            VariantPosition::Crazyhouse(pos) => SanPlus::from_move(pos, m),
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
        let variant = EffectiveVariantKey::from(self.variant.unwrap_or(VariantKey::Standard));
        let fen: Fen = self.fen.parse().map_err(|_| DestsFailure)?;
        let pos = variant.position(&fen).map_err(|_| DestsFailure)?;

        Ok(DestsResponse {
            path: self.path,
            opening: lookup_opening(fen),
            chapter_id: self.chapter_id,
            dests: dests(pos.borrow()),
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

#[derive(Deserialize)]
pub struct PlayMove {
    #[serde(deserialize_with = "util::parsable")]
    orig: Square,
    #[serde(deserialize_with = "util::parsable")]
    dest: Square,
    variant: Option<VariantKey>,
    fen: String,
    path: String,
    promotion: Option<PromotableRole>,
    #[serde(rename = "ch")]
    chapter_id: Option<String>,
}

impl PlayMove {
    pub fn respond(self) -> Result<Node, StepFailure> {
        let variant = EffectiveVariantKey::from(self.variant.unwrap_or(VariantKey::Standard));
        let fen: Fen = self.fen.parse().map_err(|_| StepFailure)?;
        let mut pos = variant.position(&fen).map_err(|_| StepFailure)?;

        let uci = Uci::Normal {
            from: self.orig,
            to: self.dest,
            promotion: self.promotion.map(|r| r.into()),
        };

        let m = pos.uci_to_move(&uci).map_err(|_| StepFailure)?;
        let san = pos.clone().san_plus(&m);
        pos.borrow_mut().play_unchecked(&m);

        Ok(Node {
            node: Branch {
                children: Vec::new(),
                san: san.to_string(),
                uci: uci.to_string(),
                id: uci_char_pair(self.orig, self.dest),
                dests: dests(pos.borrow()),
                check: Some(pos.borrow().is_check()).filter(|c| *c),
                fen: pos.fen(),
                ply: pos.borrow().fullmoves() * 2 - pos.borrow().turn().fold(1, 0),
                opening: lookup_opening(fen_from_setup(pos.borrow())),
            },
            path: "".to_owned(),
            chapter_id: self.chapter_id
        })
    }
}

#[derive(Deserialize)]
pub struct PlayDrop {
    //role: Role,
    //pos: Square,
    variant: Option<VariantKey>,
    fen: String,
    path: String,
    chapter_id: Option<String>,
}

impl PlayDrop {
    pub fn respond(self) -> Result<Node, StepFailure> {
        unimplemented!()
    }
}

#[derive(Serialize)]
pub struct Node {
    node: Branch,
    path: String,
    #[serde(rename = "ch", flatten)]
    chapter_id: Option<String>,
}

#[derive(Serialize)]
pub struct Branch {
    id: ArrayString<[u8; 2]>,
    uci: String,
    san: String,
    children: Vec<()>,
    ply: u32,
    fen: String,
    #[serde(flatten)]
    check: Option<bool>,
    dests: String,
    opening: Option<&'static Opening>,
    //drops: String, // TODO: ???
    //crazy_data: String, TODO // ???
}

#[derive(Debug)]
pub struct StepFailure;

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
