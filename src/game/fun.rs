//! The toys: Overdrive, hit-stop, word links, elite hazards and the power-ups
//! and golden ones drop.

use super::*;
use crate::hazards::{Pickup, ELITES, FREEZE_SECS, GOLDEN_ONE_IN, NUKE_LETTERS};

/// Overdrive charge needed to fire it (one point per clean letter).
pub(super) const DRIVE_FULL: f32 = 60.0;
/// Seconds Overdrive lasts before Afterburn.
const OVERDRIVE_SECS: f32 = 5.0;
/// Hazard speed during Overdrive.
pub(super) const OVERDRIVE_PACE: f32 = 0.45;
/// Charge lost to a wrong key.
const DRIVE_SLIP: f32 = 4.0;
/// Seconds Overdrive stops the boss clock.
const BOSS_HOLD: f32 = 3.0;
/// Most links that still raise the bonus.
const LINK_CAP: u32 = 8;

/// Score percentage for a word that extends a link `link` times.
pub(super) fn link_bonus(link: u32) -> u32 {
    100 + 25 * link.min(LINK_CAP)
}

impl Game {
    // --- Overdrive -------------------------------------------------------

    pub fn drive_ratio(&self) -> f32 {
        (self.drive / DRIVE_FULL).clamp(0.0, 1.0)
    }

    pub fn drive_ready(&self) -> bool {
        self.drive >= DRIVE_FULL && self.overdrive <= 0.0
    }

    pub fn overdrive_secs(&self) -> f32 {
        OVERDRIVE_SECS + Upgrade::afterburn_secs(self.upgrades.level(Upgrade::Afterburn))
    }

    /// Fraction of Overdrive left, 1 down to 0.
    pub fn overdrive_ratio(&self) -> f32 {
        (self.overdrive / self.overdrive_secs()).clamp(0.0, 1.0)
    }

    /// Clean letters charge the meter; a slip drains a little.
    pub(super) fn charge_drive(&mut self, ok: bool) {
        if self.overdrive > 0.0 {
            return;
        }
        let rate = Upgrade::drive_rate(self.upgrades.level(Upgrade::Overclock));
        self.drive = if ok {
            self.drive + rate
        } else {
            self.drive - DRIVE_SLIP
        }
        .clamp(0.0, DRIVE_FULL);
    }

    /// Tab: slow the world, double the combo, hold the boss clock.
    pub fn activate_overdrive(&mut self) -> bool {
        if self.phase != Phase::Playing || self.pending_clear || !self.drive_ready() {
            return false;
        }
        self.overdrive = self.overdrive_secs();
        self.drive = 0.0;
        if self.boss.is_some() {
            self.boss_hold = BOSS_HOLD;
        }
        self.shake = self.shake.max(0.3);
        self.callout("OVERDRIVE!", 0.0, 0.9);
        self.sfx(Sfx::Overdrive);
        true
    }

    // --- weight ----------------------------------------------------------

    /// Freeze the world for a beat (skipped with reduced motion).
    pub(super) fn hit_stop(&mut self, secs: f32) {
        if !self.settings.reduced_motion {
            self.hitstop = self.hitstop.max(secs);
        }
    }

    /// Every kill lands with a tiny hit-stop; long words hit harder, with a
    /// second, wider ring and a flash of the stage floor.
    pub(super) fn kill_weight(&mut self, letters: usize, x: f32, y: f32, radius: f32) {
        if letters >= 8 {
            self.hit_stop(0.07);
            self.burst(x, y, radius * 1.6);
            self.floor_flash = 0.14;
        } else {
            self.hit_stop(0.035);
        }
    }

    // --- links -----------------------------------------------------------

    /// A typed kill whose first letter is the last letter of the previous
    /// one extends the link; anything else breaks it.
    pub(super) fn update_link(&mut self, word: &str, x: f32, y: f32) {
        let first = word.chars().next();
        if first.is_some() && first == self.last_final {
            self.link += 1;
            if let Some((lx, ly)) = self.last_kill {
                self.effects.push(Effect::Laser {
                    x0: lx,
                    y0: ly,
                    x1: x,
                    y1: y,
                    life: 0.35,
                    max: 0.35,
                });
            }
        } else {
            self.link = 0;
        }
        self.last_final = word.chars().last();
        self.last_kill = Some((x, y));
    }

    /// Would finishing `word` next extend the link?
    pub fn extends_link(&self, word: &str) -> bool {
        self.last_final
            .is_some_and(|c| word.starts_with(c) && self.phase == Phase::Playing)
    }

    // --- elites ----------------------------------------------------------

    /// What the next spawn is. Golden first, then the elite table by wave.
    pub(super) fn roll_kind(&mut self) -> Kind {
        if self.wave >= 2 && self.rng.next() % (GOLDEN_ONE_IN as u64) < 1 {
            return Kind::Golden;
        }
        for (from, kind, chance) in ELITES {
            if self.wave >= from && self.rng.f32() < chance {
                return kind;
            }
        }
        Kind::Normal
    }

    /// An armored hazard soaks the first completion: it cracks, flashes,
    /// and asks for its word again. Returns true if it absorbed the kill.
    pub(super) fn crack(&mut self, id: u32) -> bool {
        let Some(rock) = self.rocks.iter_mut().find(|r| r.id == id && r.armor > 0) else {
            return false;
        };
        rock.armor -= 1;
        rock.hit = 0.3;
        let (x, y, radius, word) = (rock.x, rock.y, rock.radius, rock.word.clone());
        self.burst(x, y, radius * 0.7);
        self.effects.push(Effect::Float {
            x,
            y: y - radius,
            life: 0.7,
            max: 0.7,
            text: "CRACK".to_string(),
        });
        self.shake = self.shake.max(0.2);
        self.clear_typed();
        self.spent = Some(Spent {
            word,
            t0: self.time,
        });
        true
    }

    /// After a hazard is removed: splitters break apart, golden ones pay out.
    pub(super) fn on_kill(&mut self, rock: &Rock) {
        match rock.kind {
            Kind::Splitter => self.split(rock),
            Kind::Golden => self.grant_pickup(rock.x, rock.y),
            _ => {}
        }
    }

    /// Two short-word fragments fly apart from a splitter.
    fn split(&mut self, rock: &Rock) {
        let speed = (rock.vx * rock.vx + rock.vy * rock.vy).sqrt().max(3.0) * 1.15;
        let heading = rock.vy.atan2(rock.vx);
        for side in [-1.0f32, 1.0] {
            let word = self.pick_word_from(words::SHORT, 4);
            let a = heading + side * 0.8;
            let (px, py) = (-a.sin() * side * 0.8, a.cos() * side * 0.8);
            let mut frag = self.make_rock(rock.x + px, rock.y + py, 1.4, word);
            frag.vx = a.cos() * speed;
            frag.vy = a.sin() * speed;
            self.rocks.push(frag);
        }
    }

    // --- power-ups -------------------------------------------------------

    fn grant_pickup(&mut self, x: f32, y: f32) {
        if self.nuking {
            return;
        }
        let pick = Pickup::ALL[(self.rng.next() >> 33) as usize % Pickup::ALL.len()];
        self.last_pickup = Some(pick);
        self.callout(pick.callout(), 0.0, 0.9);
        match pick {
            Pickup::Freeze => self.freeze = FREEZE_SECS,
            Pickup::Nuke => self.nuke(x, y),
            Pickup::Magnet => self.magnet = true,
            Pickup::Bubble => self.shield += 1,
        }
    }

    /// Every short word on screen detonates (as chained kills).
    fn nuke(&mut self, x: f32, y: f32) {
        self.nuking = true;
        let small: Vec<(u32, String, f32, f32, f32)> = self
            .rocks
            .iter()
            .filter(|r| r.live && r.word.len() <= NUKE_LETTERS)
            .map(|r| (r.id, r.word.clone(), r.radius, r.x, r.y))
            .collect();
        for (id, word, radius, rx, ry) in small {
            self.effects.push(Effect::Laser {
                x0: x,
                y0: y,
                x1: rx,
                y1: ry,
                life: 0.3,
                max: 0.3,
            });
            self.detonate(id, &word, radius, rx, ry, true);
        }
        self.nuking = false;
    }
}
