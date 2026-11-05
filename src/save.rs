//! Run save files. The format is plain `key=value` lines so it is easy to read
//! and to keep stable; unknown keys are ignored. Nothing here touches the
//! filesystem except `path()`.

use crate::upgrades::{Upgrade, Upgrades};
use std::path::PathBuf;

/// Everything needed to resume a run at the start of a wave or in a shop.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RunSave {
    pub wave: u32,
    pub score: u32,
    pub lives: u32,
    pub crystals: u32,
    pub kills: u32,
    pub revive_used: bool,
    /// Resume in the shop (saved between waves) rather than at the wave start.
    pub in_shop: bool,
    pub upgrades: Upgrades,
    /// Extra rack slots earned by beating bosses.
    pub rack_bonus: u32,
    /// Heat level the run is played at (0 = normal).
    pub heat: u8,
    /// Words finished by typing (Sweeper counts these).
    pub typed_kills: u32,
    /// Day number of a daily run; 0 for a normal run.
    pub daily: u32,
    /// The final boss is beaten and the run continues in endless mode.
    pub won: bool,
}

pub fn encode(run: &RunSave) -> String {
    let mut out = format!(
        "wave={}\nscore={}\nlives={}\ncrystals={}\nkills={}\nrevive_used={}\nin_shop={}\n",
        run.wave,
        run.score,
        run.lives,
        run.crystals,
        run.kills,
        run.revive_used as u8,
        run.in_shop as u8,
    );
    out.push_str(&format!(
        "rack={}\nheat={}\ntyped={}\ndaily={}\nwon={}\n",
        run.rack_bonus, run.heat, run.typed_kills, run.daily, run.won as u8
    ));
    for (up, level) in run.upgrades.held() {
        out.push_str(&format!("up.{}={}\n", up.def().key, level));
    }
    out
}

/// Returns `None` for anything that is not a plausible run.
pub fn decode(text: &str) -> Option<RunSave> {
    let mut run = RunSave {
        wave: 0,
        ..RunSave::default()
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let num = value.parse::<u32>().ok();
        match key {
            "wave" => run.wave = num?,
            "score" => run.score = num?,
            "lives" => run.lives = num?,
            "crystals" => run.crystals = num?,
            "kills" => run.kills = num?,
            "revive_used" => run.revive_used = num? != 0,
            "in_shop" => run.in_shop = num? != 0,
            "rack" => run.rack_bonus = num?.min(16),
            "heat" => run.heat = num?.min(5) as u8,
            "typed" => run.typed_kills = num?,
            "daily" => run.daily = num?,
            "won" => run.won = num? != 0,
            _ => {
                if let Some(up) = key.strip_prefix("up.").and_then(Upgrade::from_key) {
                    run.upgrades.set(up, num?.min(255) as u8);
                }
            }
        }
    }
    let sane = run.wave >= 1 && run.lives >= 1;
    sane.then_some(run)
}

pub fn path() -> Option<PathBuf> {
    home_file(".typoids-run")
}

/// A file in the player's home directory.
pub fn home_file(name: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_survives_a_round_trip() {
        let mut upgrades = Upgrades::default();
        upgrades.set(Upgrade::Multishot, 2);
        upgrades.set(Upgrade::SecondWind, 1);
        let run = RunSave {
            wave: 4,
            score: 1200,
            lives: 2,
            crystals: 17,
            kills: 23,
            revive_used: true,
            in_shop: true,
            upgrades,
            rack_bonus: 2,
            heat: 3,
            typed_kills: 19,
            daily: 20_396,
            won: true,
        };
        assert_eq!(decode(&encode(&run)), Some(run));
    }

    #[test]
    fn old_saves_load_with_defaults_for_new_keys() {
        let run = decode("wave=3\nlives=2\nscore=10\n").unwrap();
        assert_eq!((run.rack_bonus, run.heat, run.daily), (0, 0, 0));
        assert!(!run.won);
    }

    #[test]
    fn retired_cards_in_old_saves_are_dropped() {
        let run = decode("wave=3\nlives=2\nup.alliteration=1\nup.focus=2\n").unwrap();
        assert_eq!(run.upgrades.level(Upgrade::Focus), 2);
        assert_eq!(run.upgrades.total_levels(), 2);
    }

    #[test]
    fn garbage_is_rejected() {
        assert_eq!(decode(""), None);
        assert_eq!(decode("wave=0\nlives=1\n"), None);
        assert_eq!(decode("wave=2\nlives=0\n"), None);
        assert_eq!(decode("wave=banana\n"), None);
    }
}
