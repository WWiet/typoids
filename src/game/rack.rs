//! Build limits and economy: the rack of installed lines, selling, shop
//! inflation, the flawless-wave bonus, and the one place hazard pace is
//! worked out.

use super::*;

/// Lines a run can hold at first; each boss beaten adds one slot.
pub(super) const RACK_BASE: u32 = 6;
pub(super) const RACK_MAX: u32 = 10;
/// Slowest that lasting slow-downs (Slow Tide, Last Stand) can make hazards.
const PACE_FLOOR: f32 = 0.5;

impl Game {
    pub fn rack_size(&self) -> u32 {
        (RACK_BASE + self.rack_bonus).min(RACK_MAX)
    }

    pub fn rack_used(&self) -> u32 {
        self.upgrades.lines_held()
    }

    /// Buying `up` needs a free slot: a new line on a full rack.
    pub(super) fn rack_full_for(&self, up: Upgrade) -> bool {
        !up.def().consumable && !self.upgrades.has(up) && self.rack_used() >= self.rack_size()
    }

    /// Tab in the shop: switch between the shelf and the upgrades screen,
    /// which describes every installed line and sells them.
    pub fn shop_toggle_rack(&mut self) {
        if self.phase != Phase::Shop {
            return;
        }
        if self.rack_used() == 0 {
            self.rack_focus = false;
            self.shop_note = "Nothing installed yet.".to_string();
            return;
        }
        self.rack_focus = !self.rack_focus;
        self.rack_sel = self.rack_sel.min(self.rack_used() as usize - 1);
        self.shop_note = if self.rack_focus {
            "Your upgrades. Enter sells the highlighted one; tab goes back.".to_string()
        } else {
            String::new()
        };
    }

    pub fn rack_move(&mut self, delta: isize) {
        let n = self.rack_used() as isize;
        if n > 0 {
            self.rack_sel = (self.rack_sel as isize + delta).rem_euclid(n) as usize;
            self.sfx(Sfx::Hover);
        }
    }

    /// The installed line under the rack cursor.
    pub fn rack_selected(&self) -> Option<(Upgrade, u8)> {
        self.upgrades.held().nth(self.rack_sel)
    }

    /// Sell the selected line for half its last price and free its slot.
    pub fn shop_sell(&mut self) {
        if self.phase != Phase::Shop || !self.rack_focus {
            return;
        }
        let Some((up, level)) = self.rack_selected() else {
            return;
        };
        let refund = up.sell_value(level);
        self.crystals += refund;
        self.upgrades.set(up, 0);
        self.lives = self.lives.min(self.max_lives()).max(1);
        // Cards on the shelf for this line go back to its first tier.
        if let Some(shop) = self.shop.as_mut() {
            for offer in shop
                .offers
                .iter_mut()
                .filter(|o| o.upgrade == up && !o.sold)
            {
                offer.tier = 1;
            }
        }
        self.shop_note = format!("Sold {} for {refund} crystals.", up.tier(level).name);
        self.sfx(Sfx::Buy);
        if self.rack_used() == 0 {
            self.rack_focus = false;
        } else {
            self.rack_sel = self.rack_sel.min(self.rack_used() as usize - 1);
        }
        self.autosave(true);
    }

    /// Every card costs one more crystal per two tiers held.
    pub fn inflation(&self) -> u32 {
        self.upgrades.total_levels() / 2
    }

    /// Crystals for clearing a wave without losing a heart.
    pub(super) fn flawless_bonus(&self) -> u32 {
        2 + self.wave / 3
    }

    /// How fast hazards move right now, before Focus. Lasting slow-downs
    /// share a floor; Brake Pads, Overdrive, Freeze and heat stack on top.
    pub fn hazard_pace(&self) -> f32 {
        if self.freeze > 0.0 {
            return 0.0;
        }
        let mut lasting = 1.0 - 0.1 * self.upgrades.level(Upgrade::SlowTide) as f32;
        if self.lives == 1 && self.upgrades.has(Upgrade::LastStand) {
            lasting *= 0.8;
        }
        let mut pace = lasting.max(PACE_FLOOR);
        if self.brake > 0.0 {
            pace *= 0.35;
        }
        if self.overdrive > 0.0 {
            pace *= fun::OVERDRIVE_PACE;
        }
        if self.heat_rule(1) {
            pace *= arcade::HEAT_SPEED;
        }
        pace
    }
}
