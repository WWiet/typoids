//! Elite hazards and the power-ups golden hazards drop. Data only; `game.rs`
//! decides what each kind does and `draw.rs` marks them on the word chip.

/// What a hazard does beyond carrying a word.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Kind {
    #[default]
    Normal,
    /// Breaks into two short-word fragments when destroyed.
    Splitter,
    /// A short word on a fast hazard.
    Runner,
    /// Must be typed twice: the first time cracks the armor.
    Armored,
    /// Only the first letters show until you lock it.
    Masked,
    /// Slow and shiny; destroying it grants a power-up.
    Golden,
}

impl Kind {
    /// The mark printed in front of the word chip.
    pub fn mark(self, armor: u8) -> Option<&'static str> {
        match self {
            Kind::Normal => None,
            Kind::Splitter => Some("÷"),
            Kind::Runner => Some("»"),
            Kind::Armored if armor > 0 => Some("×2"),
            Kind::Armored => None,
            Kind::Masked => Some("?"),
            Kind::Golden => Some("◆"),
        }
    }

    /// Speed multiplier on spawn.
    pub fn speed(self) -> f32 {
        match self {
            Kind::Runner => 1.8,
            Kind::Golden => 0.7,
            Kind::Armored => 0.85,
            _ => 1.0,
        }
    }
}

/// When elites start to appear and how often: (first wave, kind, chance per
/// spawn). Rolled in order; the first hit wins, otherwise the hazard is normal.
pub const ELITES: [(u32, Kind, f32); 4] = [
    (3, Kind::Splitter, 0.14),
    (6, Kind::Runner, 0.12),
    (9, Kind::Armored, 0.10),
    (13, Kind::Masked, 0.10),
];

/// One spawn in this many is golden (from wave 2, never on boss waves).
pub const GOLDEN_ONE_IN: u32 = 18;

/// Letters a masked hazard shows before you lock it.
pub const MASK_SHOWN: usize = 2;

/// A power-up from a golden hazard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pickup {
    /// Every hazard stops for a few seconds.
    Freeze,
    /// Every word of four letters or fewer detonates.
    Nuke,
    /// Crystals fly to the ship at once for the rest of the wave.
    Magnet,
    /// One extra ward for this wave.
    Bubble,
}

impl Pickup {
    pub const ALL: [Pickup; 4] = [Pickup::Freeze, Pickup::Nuke, Pickup::Magnet, Pickup::Bubble];

    /// The call-out shown when it is picked up.
    pub fn callout(self) -> &'static str {
        match self {
            Pickup::Freeze => "FREEZE!",
            Pickup::Nuke => "NUKE!",
            Pickup::Magnet => "MAGNET!",
            Pickup::Bubble => "BUBBLE!",
        }
    }
}

/// Seconds Freeze holds every hazard still.
pub const FREEZE_SECS: f32 = 3.0;
/// Longest word a Nuke pops.
pub const NUKE_LETTERS: usize = 4;
