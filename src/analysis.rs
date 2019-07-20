use std::mem;

use serde::{Deserialize, Serialize};

use arrayvec::ArrayString;

use shakmaty::{Square, Castles, PositionError, Setup, Position, MoveList, Role, IllegalMoveError, File, MaterialSide, Material};
use shakmaty::variants::{Variant, VariantPosition};
use shakmaty::fen::{Fen, FenOpts, ParseFenError};
use shakmaty::san::SanPlus;
use shakmaty::uci::Uci;
use shakmaty::attacks;

use crate::util;

#[derive(Serialize)]
pub struct Opening {
    eco: &'static str,
    name: &'static str,
}

fn lookup_opening(mut fen: Fen) -> Option<&'static Opening> {
    fen.pockets = None;
    fen.remaining_checks = None;
    OPENING_DB.get(FenOpts::new().epd(&fen).as_str())
}

fn uci_char_pair(uci: &Uci) -> ArrayString<[u8; 3]> {
    let mut r = ArrayString::new();
    match *uci {
        Uci::Normal { from, to, promotion: None } => {
            r.push(square_id(from));
            r.push(square_id(to));
        }
        Uci::Normal { from, to, promotion: Some(role) } => {
            r.push(square_id(from));
            r.push(promotion_id(to.file(), role));
        }
        Uci::Put { to, role } => {
            r.push(square_id(to));
            r.push(drop_role_id(role));
        }
        Uci::Null => {
            r.push(35 as char);
            r.push(35 as char);
        }
    }
    r
}

fn square_id(sq: Square) -> char {
    (u8::from(sq) + 35) as char
}

fn drop_role_id(role: Role) -> char {
    (35 + 64 + 8 * 5 + match role {
        Role::King => 0u8, // cannot be dropped
        Role::Queen => 0,
        Role::Rook => 1,
        Role::Bishop => 2,
        Role::Knight => 3,
        Role::Pawn => 4,
    }) as char
}

fn promotion_id(file: File, role: Role) -> char {
    (35 + 64 + (match role {
        Role::Queen => 0,
        Role::Rook => 1,
        Role::Bishop => 2,
        Role::Knight => 3,
        Role::King => 4,
        Role::Pawn => 0,
    }) * 8 + u8::from(file)) as char
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

fn dests(pos: &VariantPosition) -> String {
    let mut legals = MoveList::new();
    pos.legal_moves(&mut legals);

    let mut dests = String::with_capacity(80);
    let mut first = true;
    for from_sq in pos.us() {
        let mut from_here = legals.iter().filter(|m| m.from() == Some(from_sq)).peekable();
        if from_here.peek().is_some() {
            if !mem::replace(&mut first, false) {
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

fn drops(pos: &VariantPosition) -> Option<String> {
    let checkers = pos.checkers();

    if checkers.is_empty() || pos.pockets().map_or(true, |p| p.by_color(pos.turn()).is_empty()) {
        None
    } else if let Some(checker) = checkers.single_square() {
        let king = pos.board().king_of(pos.turn()).expect("king in crazyhouse");
        Some(attacks::between(checker, king).into_iter().map(|sq| sq.to_string()).collect())
    } else {
        Some("".to_owned())
    }
}

fn fix_castles(variant: Variant, fen: &mut Fen) {
    if variant == Variant::RacingKings {
        fen.castling_rights.clear();
    } else if let Err(filtered_castles) = Castles::from_setup(fen) {
        fen.castling_rights = filtered_castles.castling_rights();
    }
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

#[derive(Deserialize)]
enum DroppableRole {
    #[serde(rename = "pawn")]
    Pawn,
    #[serde(rename = "knight")]
    Knight,
    #[serde(rename = "bishop")]
    Bishop,
    #[serde(rename = "rook")]
    Rook,
    #[serde(rename = "queen")]
    Queen,
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

impl From<DroppableRole> for Role {
    fn from(r: DroppableRole) -> Role {
        match r {
            DroppableRole::Pawn => Role::Pawn,
            DroppableRole::Knight => Role::Knight,
            DroppableRole::Bishop => Role::Bishop,
            DroppableRole::Rook => Role::Rook,
            DroppableRole::Queen => Role::Queen,
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

impl From<VariantKey> for Variant {
    fn from(variant: VariantKey) -> Variant {
        match variant {
            VariantKey::Standard | VariantKey::FromPosition | VariantKey::Chess960 => Variant::Chess,
            VariantKey::Antichess => Variant::Giveaway,
            VariantKey::KingOfTheHill => Variant::KingOfTheHill,
            VariantKey::ThreeCheck => Variant::ThreeCheck,
            VariantKey::Atomic => Variant::Atomic,
            VariantKey::Horde => Variant::Horde,
            VariantKey::RacingKings => Variant::RacingKings,
            VariantKey::Crazyhouse => Variant::Crazyhouse,
        }
    }
}

fn is_opening_sensible(variant: Variant) -> bool {
    match variant {
        Variant::Chess | Variant::Crazyhouse | Variant::ThreeCheck | Variant::KingOfTheHill => true,
        _ => false,
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
        let variant = Variant::from(self.variant.unwrap_or(VariantKey::Standard));
        self.fen.parse().ok()
            .filter(|_| is_opening_sensible(variant))
            .and_then(lookup_opening)
            .map(|opening| OpeningResponse {
                path: self.path,
                opening
            })
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
    pub fn respond(self) -> Result<DestsResponse, StepFailure> {
        let variant = Variant::from(self.variant.unwrap_or(VariantKey::Standard));
        let mut fen: Fen = self.fen.parse()?;
        fix_castles(variant, &mut fen);
        let pos = VariantPosition::from_setup(variant, &fen)?;

        Ok(DestsResponse {
            path: self.path,
            opening: lookup_opening(fen).filter(|_| is_opening_sensible(variant)),
            chapter_id: self.chapter_id,
            dests: dests(&pos),
            drops: drops(&pos),
        })
    }
}

#[derive(Serialize)]
pub struct DestsResponse {
    path: String,
    dests: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    drops: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opening: Option<&'static Opening>,
    #[serde(rename = "ch", skip_serializing_if = "Option::is_none")]
    chapter_id: Option<String>,
}

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

#[derive(Deserialize)]
pub struct PlayDrop {
    role: DroppableRole,
    #[serde(deserialize_with = "util::parsable")]
    pos: Square,
    variant: Option<VariantKey>,
    fen: String,
    path: String,
    #[serde(rename = "ch")]
    chapter_id: Option<String>,
}

pub struct PlayStep {
    uci: Uci,
    variant: Option<VariantKey>,
    fen: String,
    path: String,
    chapter_id: Option<String>,
}

impl From<PlayMove> for PlayStep {
    fn from(d: PlayMove) -> PlayStep {
        PlayStep {
            uci: Uci::Normal {
                from: d.orig,
                to: d.dest,
                promotion: d.promotion.map(Role::from),
            },
            variant: d.variant,
            fen: d.fen,
            path: d.path,
            chapter_id: d.chapter_id,
        }
    }
}

impl From<PlayDrop> for PlayStep {
    fn from(d: PlayDrop) -> PlayStep {
        PlayStep {
            uci: Uci::Put {
                to: d.pos,
                role: d.role.into(),
            },
            variant: d.variant,
            fen: d.fen,
            path: d.path,
            chapter_id: d.chapter_id,
        }
    }
}

impl PlayStep {
    pub fn respond(self) -> Result<Node, StepFailure> {
        let variant = Variant::from(self.variant.unwrap_or(VariantKey::Standard));
        let mut fen: Fen = self.fen.parse()?;
        fix_castles(variant, &mut fen);
        let mut pos = VariantPosition::from_setup(variant, &fen)?;

        let m = self.uci.to_move(&pos)?;
        let san = SanPlus::from_move_and_play_unchecked(&mut pos, &m);

        Ok(Node {
            node: Branch {
                children: Vec::new(),
                san: san.to_string(),
                uci: self.uci.to_string(),
                id: uci_char_pair(&self.uci),
                dests: dests(&pos),
                drops: drops(&pos),
                check: pos.is_check(),
                fen: FenOpts::default().scid(true).fen(&pos),
                ply: (pos.fullmoves() - 1) * 2 + pos.turn().fold(0, 1),
                opening: lookup_opening(Fen::from_setup(&pos)).filter(|_| is_opening_sensible(variant)),
                crazy: pos.pockets().map(CrazyData::from)
            },
            path: self.path,
            chapter_id: self.chapter_id
        })
    }
}

#[derive(Serialize)]
pub struct Node {
    node: Branch,
    path: String,
    #[serde(rename = "ch", skip_serializing_if = "Option::is_none")]
    chapter_id: Option<String>,
}

#[derive(Serialize)]
pub struct Branch {
    id: ArrayString<[u8; 3]>,
    uci: String,
    san: String,
    children: Vec<()>,
    ply: u32,
    fen: String,
    #[serde(skip_serializing_if = "util::is_false")]
    check: bool,
    dests: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    opening: Option<&'static Opening>,
    #[serde(skip_serializing_if = "Option::is_none")]
    drops: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    crazy: Option<CrazyData>,
}

#[derive(Serialize)]
pub struct CrazyData {
    pockets: [CrazyPocket; 2]
}

impl<'a> From<&'a Material> for CrazyData {
    fn from(material: &'a Material) -> CrazyData {
        CrazyData {
            pockets: [(&material.white).into(), (&material.black).into()]
        }
    }
}

#[derive(Serialize)]
pub struct CrazyPocket {
    #[serde(skip_serializing_if = "util::is_zero_u8")]
    pawn: u8,
    #[serde(skip_serializing_if = "util::is_zero_u8")]
    knight: u8,
    #[serde(skip_serializing_if = "util::is_zero_u8")]
    bishop: u8,
    #[serde(skip_serializing_if = "util::is_zero_u8")]
    rook: u8,
    #[serde(skip_serializing_if = "util::is_zero_u8")]
    queen: u8,
}

impl<'a> From<&'a MaterialSide> for CrazyPocket {
    fn from(side: &'a MaterialSide) -> CrazyPocket {
        CrazyPocket {
            pawn: side.pawns,
            knight: side.knights,
            bishop: side.bishops,
            rook: side.rooks,
            queen: side.queens,
        }
    }
}

#[derive(Debug)]
pub enum StepFailure {
    ParseFenError(ParseFenError),
    PositionError(PositionError),
    IllegalMoveError(IllegalMoveError),
}

impl From<ParseFenError> for StepFailure {
    fn from(err: ParseFenError) -> StepFailure {
        StepFailure::ParseFenError(err)
    }
}

impl From<PositionError> for StepFailure {
    fn from(err: PositionError) -> StepFailure {
        StepFailure::PositionError(err)
    }
}

impl From<IllegalMoveError> for StepFailure {
    fn from(err: IllegalMoveError) -> StepFailure {
        StepFailure::IllegalMoveError(err)
    }
}

include!(concat!(env!("OUT_DIR"), "/opening_db.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piotr() {
        assert_eq!(piotr(Square::A1), 'a');
        assert_eq!(piotr(Square::B4), 'z');
        assert_eq!(piotr(Square::C4), 'A');
        assert_eq!(piotr(Square::D7), 'Z');
        assert_eq!(piotr(Square::E7), '0');
        assert_eq!(piotr(Square::F8), '9');
        assert_eq!(piotr(Square::G8), '!');
        assert_eq!(piotr(Square::H8), '?');
    }

    #[test]
    fn test_uci_char_pair() {
        // regular moves
        assert_eq!(&uci_char_pair(&Uci::Normal { from: Square::A1, to: Square::B1, promotion: None }), "#$");
        assert_eq!(&uci_char_pair(&Uci::Normal { from: Square::A1, to: Square::A2, promotion: None }), "#+");
        assert_eq!(&uci_char_pair(&Uci::Normal { from: Square::H7, to: Square::H8, promotion: None }), "Zb");

        // promotions
        assert_eq!(&uci_char_pair(&Uci::Normal { from: Square::B7, to: Square::B8, promotion: Some(Role::Queen) }), "Td");
        assert_eq!(&uci_char_pair(&Uci::Normal { from: Square::B7, to: Square::C8, promotion: Some(Role::Queen) }), "Te");
        assert_eq!(&uci_char_pair(&Uci::Normal { from: Square::B7, to: Square::C8, promotion: Some(Role::Knight) }), "T}");

        // drops
        assert_eq!(&uci_char_pair(&Uci::Put { to: Square::A1, role: Role::Pawn }), "#\u{8f}");
        assert_eq!(&uci_char_pair(&Uci::Put { to: Square::H8, role: Role::Queen }), "b\u{8b}");
    }
}
