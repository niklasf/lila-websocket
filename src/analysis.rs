#[derive(Deserialize)]
struct VariantKey {
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

struct EffectiveVariantKey {
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
    fn opening_sensible(self) -> bool {
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
struct GetOpening {
    variant: Option<VariantKey>,
    path: String,
    fen: String,
}
