//! The roguelike upgrade catalog. Pure data plus tiny helpers; the simulation
//! in `game.rs` decides what each upgrade does.
//!
//! An `Upgrade` is a *line* of cards. Each level of a line is its own card with
//! its own name and rarity (`Def::tiers`), and the shop only ever offers the
//! next one, so a bought card never restocks itself: the next tier has to
//! roll in on a later shop or reroll.

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
            Rarity::Common => 8,
            Rarity::Uncommon => 16,
            Rarity::Rare => 28,
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
    SecondWind,
    Focus,
    BigGame,
    Haggler,
    Leech,
    Scattershot,
    BrakePads,
    LuckyPenny,
    CleanSlate,
    Shrapnel,
    LastStand,
    Recycler,
    Bounty,
    Sweeper,
    Discount,
    WideShop,
    Overclock,
    Afterburn,
}

pub const COUNT: usize = 27;

/// One level of a line: the card the shop shows for it.
pub struct Tier {
    pub name: &'static str,
    pub rarity: Rarity,
}

pub struct Def {
    /// Stable id used in save files.
    pub key: &'static str,
    /// Card text for single-level lines; stacking lines use `Upgrade::blurb`.
    pub desc: &'static str,
    /// One card per level, in order. The line's maximum level is `tiers.len()`.
    pub tiers: &'static [Tier],
    /// Applied once on purchase and never held.
    pub consumable: bool,
}

use Rarity::{Common as C, Rare as R, Uncommon as U};

const fn t(name: &'static str, rarity: Rarity) -> Tier {
    Tier { name, rarity }
}

const fn def(key: &'static str, desc: &'static str, tiers: &'static [Tier]) -> Def {
    Def {
        key,
        desc,
        tiers,
        consumable: false,
    }
}

/// Indexed by `Upgrade as usize`; keep in the same order as the enum.
const DEFS: [Def; COUNT] = [
    def(
        "steadyhands",
        "Words with no slips: more score and +1 crystal.",
        &[t("STEADY HANDS", C), t("IRON NERVES", U), t("METRONOME", R)],
    ),
    def(
        "prospector",
        "Kills may drop an extra crystal.",
        &[
            t("PROSPECTOR", C),
            t("GOLD PAN", C),
            t("PICKAXE", U),
            t("MINING RIG", U),
            t("MOTHERLODE", R),
        ],
    ),
    def(
        "slowtide",
        "Hazards drift slower.",
        &[
            t("SLOW TIDE", C),
            t("DEEP CURRENT", C),
            t("STILL WATER", U),
            t("DEAD CALM", U),
            t("FROZEN TIME", R),
        ],
    ),
    def(
        "longfuse",
        "Combos last longer.",
        &[
            t("LONG FUSE", C),
            t("SLOW BURN", C),
            t("STEADY FLAME", U),
            t("INFERNO", R),
        ],
    ),
    Def {
        consumable: true,
        ..def("patchkit", "Restore one heart.", &[t("PATCH KIT", C)])
    },
    def(
        "extraheart",
        "+1 max heart, and heal one.",
        &[t("EXTRA HEART", U), t("SPARE HEART", U), t("IRON HEART", R)],
    ),
    def(
        "multishot",
        "Kills also pop the nearest short word.",
        &[
            t("MULTISHOT", U),
            t("LONG BARREL", U),
            t("WIDE BORE", R),
            t("GRAND SLAM", R),
        ],
    ),
    def(
        "ward",
        "Absorb hits at the start of each wave.",
        &[t("WARD", U), t("GUARD", U), t("BULWARK", R)],
    ),
    def(
        "interest",
        "Earn 1 crystal per 5 held after each wave (max 5).",
        &[t("INTEREST", R)],
    ),
    def(
        "secondwind",
        "Survive your first fatal hit with one heart.",
        &[t("SECOND WIND", R)],
    ),
    def(
        "focus",
        "The word you are typing slows down.",
        &[
            t("FOCUS", U),
            t("BLINKERS", R),
            t("LASER FOCUS", R),
            t("ZEN MASTER", R),
        ],
    ),
    def(
        "biggame",
        "Words of 7+ letters: 2x score, +2 crystals.",
        &[t("BIG GAME", R)],
    ),
    def(
        "haggler",
        "The first reroll in every shop is free.",
        &[t("HAGGLER", R)],
    ),
    def(
        "leech",
        "Every 12 kills, restore one heart.",
        &[t("LEECH", R)],
    ),
    def(
        "scattershot",
        "Multishot pops more words per kill.",
        &[t("SCATTERSHOT", R), t("BUCKSHOT", R), t("FLAK STORM", R)],
    ),
    def(
        "brakepads",
        "Hazards crawl at the start of each wave.",
        &[t("BRAKE PADS", C), t("DISC BRAKES", U)],
    ),
    def(
        "luckypenny",
        "Some kills drop double crystals.",
        &[t("LUCKY PENNY", C), t("LUCKY CLOVER", U)],
    ),
    def(
        "cleanslate",
        "The first wrong key each wave is forgiven.",
        &[t("CLEAN SLATE", U)],
    ),
    def(
        "shrapnel",
        "Kills shove nearby hazards away.",
        &[t("SHRAPNEL", U), t("CLUSTER BOMB", R)],
    ),
    def(
        "laststand",
        "At one heart: hazards 20% slower, score doubled.",
        &[t("LAST STAND", U)],
    ),
    def(
        "recycler",
        "Rerolls refund some crystals.",
        &[t("RECYCLER", U), t("SCRAP YARD", R)],
    ),
    def(
        "bounty",
        "Kills at combo 5 or more drop one more crystal.",
        &[t("BOUNTY", R)],
    ),
    def(
        "sweeper",
        "Every 25th word you type detonates every hazard.",
        &[t("SWEEPER", R)],
    ),
    def(
        "discount",
        "Shop prices drop (never below 1).",
        &[t("DISCOUNT", R), t("LOYALTY CARD", R)],
    ),
    def(
        "wideshop",
        "The shop offers a fourth card.",
        &[t("WIDE SHOP", R)],
    ),
    def(
        "overclock",
        "The Overdrive meter fills 30% faster.",
        &[t("OVERCLOCK", U)],
    ),
    def(
        "afterburn",
        "Overdrive lasts 2 seconds longer.",
        &[t("AFTERBURN", R)],
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
        Upgrade::SecondWind,
        Upgrade::Focus,
        Upgrade::BigGame,
        Upgrade::Haggler,
        Upgrade::Leech,
        Upgrade::Scattershot,
        Upgrade::BrakePads,
        Upgrade::LuckyPenny,
        Upgrade::CleanSlate,
        Upgrade::Shrapnel,
        Upgrade::LastStand,
        Upgrade::Recycler,
        Upgrade::Bounty,
        Upgrade::Sweeper,
        Upgrade::Discount,
        Upgrade::WideShop,
        Upgrade::Overclock,
        Upgrade::Afterburn,
    ];

    pub fn def(self) -> &'static Def {
        &DEFS[self as usize]
    }

    /// Most levels a run can hold.
    pub fn max(self) -> u8 {
        self.def().tiers.len() as u8
    }

    /// The card for `level` (1-based, clamped into range).
    pub fn tier(self, level: u8) -> &'static Tier {
        let tiers = self.def().tiers;
        &tiers[(level.max(1) as usize - 1).min(tiers.len() - 1)]
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|u| u.def().key == key)
    }

    /// Longest word Multishot can pop at `level`.
    pub fn multishot_letters(level: u8) -> usize {
        3 + level as usize
    }

    /// Pace of the typed hazard under Focus at `level` (1.0 means no effect).
    pub fn focus_pace(level: u8) -> f32 {
        1.0 / (1.0 + 0.5 * level as f32)
    }

    /// Seconds hazards crawl at the start of a wave under Brake Pads.
    pub fn brake_secs(level: u8) -> f32 {
        1.0 + 2.0 * level as f32
    }

    /// Chance that a kill drops double crystals under Lucky Penny.
    pub fn lucky_chance(level: u8) -> f32 {
        0.15 * level as f32
    }

    /// Cells within which Shrapnel shoves hazards away.
    pub fn shrapnel_radius(level: u8) -> f32 {
        3.0 + 3.0 * level as f32
    }

    /// Crystals a reroll gives back under Recycler.
    pub fn recycler_refund(level: u8) -> u32 {
        1 + 2 * level as u32
    }

    /// Crystals taken off every shop price under Discount.
    pub fn discount_off(level: u8) -> u32 {
        2 * level as u32
    }

    /// Chance per kill of one extra crystal under Prospector.
    pub fn prospector_chance(level: u8) -> f32 {
        (0.2 * level as f32).min(1.0)
    }

    /// Overdrive charge per clean letter under Overclock.
    pub fn drive_rate(level: u8) -> f32 {
        1.0 + 0.3 * level as f32
    }

    /// Extra seconds of Overdrive under Afterburn.
    pub fn afterburn_secs(level: u8) -> f32 {
        2.0 * level as f32
    }

    /// Card text for lines whose numbers scale: a template with `{}` where
    /// the number goes, and the number at a given level.
    fn scaling(self, level: u8) -> Option<(&'static str, String)> {
        let n = level as u32;
        let pct = |f: f32| format!("{}%", (f * 100.0).round());
        Some(match self {
            Upgrade::SteadyHands => (
                "Words with no slips: +{} score, +1 crystal.",
                format!("{}%", 25 * n),
            ),
            Upgrade::Prospector => (
                "Kills have a {} chance of one extra crystal.",
                pct(Self::prospector_chance(level)),
            ),
            Upgrade::SlowTide => ("Hazards drift {} slower.", format!("{}%", 10 * n)),
            Upgrade::LongFuse => ("Combos last {} longer.", format!("{}s", 1.5 * n as f32)),
            Upgrade::Ward => (
                "Wards each wave: {}. A ward absorbs one hit.",
                n.to_string(),
            ),
            Upgrade::Multishot => (
                "Kills also pop the nearest word of {} letters or fewer.",
                Self::multishot_letters(level).to_string(),
            ),
            Upgrade::Scattershot => ("Multishot pops {} words per kill.", (1 + n).to_string()),
            Upgrade::Focus => (
                "The word you type slows to {} speed.",
                pct(Self::focus_pace(level)),
            ),
            Upgrade::BrakePads => (
                "Hazards crawl for the first {} of each wave.",
                format!("{}s", Self::brake_secs(level)),
            ),
            Upgrade::LuckyPenny => (
                "{} of kills drop double crystals.",
                pct(Self::lucky_chance(level)),
            ),
            Upgrade::Shrapnel => (
                "Kills shove hazards within {} cells away.",
                Self::shrapnel_radius(level).to_string(),
            ),
            Upgrade::Recycler => (
                "Each reroll refunds {} crystals.",
                Self::recycler_refund(level).to_string(),
            ),
            Upgrade::Discount => (
                "Shop prices are {} lower (never below 1).",
                Self::discount_off(level).to_string(),
            ),
            _ => return None,
        })
    }

    /// Card text for the level you would have after buying. Lines that scale
    /// spell out their next step; the rest use the fixed description.
    pub fn blurb(self, next_level: u8) -> String {
        let l = next_level.clamp(1, self.max());
        match self.scaling(l) {
            Some((template, value)) => template.replacen("{}", &value, 1),
            None => self.def().desc.to_string(),
        }
    }

    /// Card text for buying the next level while holding `held`: scaling
    /// lines show the step, as in "Hazards drift 20% → 30% slower."
    pub fn blurb_delta(self, held: u8) -> String {
        let next = (held + 1).min(self.max());
        match (held, self.scaling(held), self.scaling(next)) {
            (1.., Some((template, was)), Some((_, now))) => {
                template.replacen("{}", &format!("{was} → {now}"), 1)
            }
            _ => self.blurb(next),
        }
    }

    /// Crystals refunded for selling a line held at `level`.
    pub fn sell_value(self, level: u8) -> u32 {
        if level == 0 {
            return 0;
        }
        (self.cost(level - 1) / 2).max(1)
    }

    /// Crystals needed to buy the card at `owned + 1` before any discount.
    pub fn cost(self, owned: u8) -> u32 {
        if self.def().consumable {
            return 8;
        }
        self.tier(owned + 1).rarity.base_cost() + owned as u32 * 2
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
        !up.def().consumable && self.level(up) >= up.max()
    }

    /// Adds a level, clamped to the upgrade's maximum. Consumables hold none.
    pub fn add(&mut self, up: Upgrade) {
        if !up.def().consumable {
            let slot = &mut self.0[up as usize];
            *slot = (*slot + 1).min(up.max());
        }
    }

    pub fn set(&mut self, up: Upgrade, level: u8) {
        self.0[up as usize] = level.min(up.max());
    }

    /// Levels held across every line (drives shop inflation).
    pub fn total_levels(&self) -> u32 {
        self.0.iter().map(|&l| l as u32).sum()
    }

    /// Lines with at least one level (each takes a rack slot).
    pub fn lines_held(&self) -> u32 {
        self.0.iter().filter(|&&l| l > 0).count() as u32
    }

    /// Held upgrades in catalog order, with their levels.
    pub fn held(&self) -> impl Iterator<Item = (Upgrade, u8)> + '_ {
        Upgrade::ALL
            .into_iter()
            .filter(|&u| self.has(u))
            .map(|u| (u, self.level(u)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaling_cards_show_the_step_they_buy() {
        assert_eq!(
            Upgrade::SlowTide.blurb_delta(2),
            "Hazards drift 20% → 30% slower."
        );
        assert_eq!(
            Upgrade::SlowTide.blurb_delta(0),
            "Hazards drift 10% slower."
        );
        assert_eq!(Upgrade::Haggler.blurb_delta(0), Upgrade::Haggler.def().desc);
    }

    #[test]
    fn card_text_stays_short_enough_for_a_card() {
        for up in Upgrade::ALL {
            for held in 0..up.max() {
                let text = up.blurb_delta(held);
                assert!(text.chars().count() <= 62, "{text}");
            }
            for level in 1..=up.max() {
                assert!(up.tier(level).name.chars().count() <= 12);
            }
        }
    }

    #[test]
    fn motherlode_is_a_sure_thing_not_a_fortune() {
        assert_eq!(Upgrade::prospector_chance(5), 1.0);
        assert!(Upgrade::prospector_chance(1) < 0.25);
    }

    #[test]
    fn selling_refunds_half_the_last_price() {
        assert_eq!(
            Upgrade::SlowTide.sell_value(1),
            Upgrade::SlowTide.cost(0) / 2
        );
        assert_eq!(Upgrade::SlowTide.sell_value(0), 0);
    }
}
