//! The roguelike upgrade catalog. Pure data plus tiny helpers; the simulation
//! in `game.rs` decides what each upgrade does.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
}

impl Rarity {
    pub fn label(self) -> &'static str {
        match self {
            Rarity::Common => "COMMON",
            Rarity::Uncommon => "UNCOMMON",
            Rarity::Rare => "RARE",
        }
    }

    /// Relative odds of appearing in a shop slot.
    pub fn weight(self) -> f32 {
        match self {
            Rarity::Common => 6.0,
            Rarity::Uncommon => 3.0,
            Rarity::Rare => 1.2,
        }
    }

    fn base_cost(self) -> u32 {
        match self {
            Rarity::Common => 6,
            Rarity::Uncommon => 10,
            Rarity::Rare => 16,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Upgrade {
    SteadyHands,
    Prospector,
    SlowTide,
    LongFuse,
    PatchKit,
    ExtraHeart,
    Multishot,
    Ward,
    Interest,
    Alliteration,
    SecondWind,
    Focus,
    BigGame,
    Haggler,
    Leech,
}

pub const COUNT: usize = 15;

pub struct Def {
    /// Stable id used in save files.
    pub key: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub rarity: Rarity,
    /// Most levels a run can hold.
    pub max: u8,
    /// Applied once on purchase and never held.
    pub consumable: bool,
}

const fn def(
    key: &'static str,
    name: &'static str,
    desc: &'static str,
    rarity: Rarity,
    max: u8,
) -> Def {
    Def {
        key,
        name,
        desc,
        rarity,
        max,
        consumable: false,
    }
}

/// Indexed by `Upgrade as usize`; keep in the same order as the enum.
const DEFS: [Def; COUNT] = [
    def(
        "steadyhands",
        "STEADY HANDS",
        "Words typed with no slips: +25% score, +1 crystal.",
        Rarity::Common,
        3,
    ),
    def(
        "prospector",
        "PROSPECTOR",
        "Every kill drops one more crystal.",
        Rarity::Common,
        5,
    ),
    def(
        "slowtide",
        "SLOW TIDE",
        "Hazards drift 10% slower.",
        Rarity::Common,
        5,
    ),
    def(
        "longfuse",
        "LONG FUSE",
        "Combos last 1.5s longer.",
        Rarity::Common,
        4,
    ),
    Def {
        consumable: true,
        ..def(
            "patchkit",
            "PATCH KIT",
            "Restore one heart.",
            Rarity::Common,
            255,
        )
    },
    def(
        "extraheart",
        "EXTRA HEART",
        "+1 max heart, and heal one.",
        Rarity::Uncommon,
        3,
    ),
    def(
        "multishot",
        "MULTISHOT",
        "Kills also pop the nearest word of 4 or fewer letters.",
        Rarity::Uncommon,
        3,
    ),
    def(
        "ward",
        "WARD",
        "Absorb one hit at the start of each wave.",
        Rarity::Uncommon,
        3,
    ),
    def(
        "interest",
        "COMPOUND INTEREST",
        "Earn 1 crystal per 5 held after each wave (max 5).",
        Rarity::Rare,
        1,
    ),
    def(
        "alliteration",
        "ALLITERATION",
        "Same first letter twice in a row: 2x score, +1 crystal.",
        Rarity::Rare,
        1,
    ),
    def(
        "secondwind",
        "SECOND WIND",
        "Survive your first fatal hit with one heart.",
        Rarity::Rare,
        1,
    ),
    def(
        "focus",
        "FOCUS",
        "Hazards slow by a third while you type.",
        Rarity::Rare,
        1,
    ),
    def(
        "biggame",
        "BIG GAME",
        "Words of 7+ letters: 2x score, +2 crystals.",
        Rarity::Rare,
        1,
    ),
    def(
        "haggler",
        "HAGGLER",
        "The first reroll in every shop is free.",
        Rarity::Rare,
        1,
    ),
    def(
        "leech",
        "LEECH",
        "Every 12 kills, restore one heart.",
        Rarity::Rare,
        1,
    ),
];

impl Upgrade {
    pub const ALL: [Upgrade; COUNT] = [
        Upgrade::SteadyHands,
        Upgrade::Prospector,
        Upgrade::SlowTide,
        Upgrade::LongFuse,
        Upgrade::PatchKit,
        Upgrade::ExtraHeart,
        Upgrade::Multishot,
        Upgrade::Ward,
        Upgrade::Interest,
        Upgrade::Alliteration,
        Upgrade::SecondWind,
        Upgrade::Focus,
        Upgrade::BigGame,
        Upgrade::Haggler,
        Upgrade::Leech,
    ];

    pub fn def(self) -> &'static Def {
        &DEFS[self as usize]
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|u| u.def().key == key)
    }

    /// Crystals needed to buy the next level when `owned` levels are held.
    pub fn cost(self, owned: u8) -> u32 {
        if self.def().consumable {
            return 5;
        }
        self.def().rarity.base_cost() + owned as u32 * 2
    }
}

/// Levels held per upgrade for the current run.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Upgrades([u8; COUNT]);

impl Upgrades {
    pub fn level(&self, up: Upgrade) -> u8 {
        self.0[up as usize]
    }

    pub fn has(&self, up: Upgrade) -> bool {
        self.level(up) > 0
    }

    pub fn maxed(&self, up: Upgrade) -> bool {
        !up.def().consumable && self.level(up) >= up.def().max
    }

    /// Adds a level, clamped to the upgrade's maximum. Consumables hold none.
    pub fn add(&mut self, up: Upgrade) {
        if !up.def().consumable {
            let slot = &mut self.0[up as usize];
            *slot = (*slot + 1).min(up.def().max);
        }
    }

    pub fn set(&mut self, up: Upgrade, level: u8) {
        self.0[up as usize] = level.min(up.def().max);
    }

    /// Held upgrades in catalog order, with their levels.
    pub fn held(&self) -> impl Iterator<Item = (Upgrade, u8)> + '_ {
        Upgrade::ALL
            .into_iter()
            .filter(|&u| self.has(u))
            .map(|u| (u, self.level(u)))
    }
}
