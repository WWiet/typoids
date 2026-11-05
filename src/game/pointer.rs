//! Mouse support: the things a click can land on, and selecting them the way
//! the arrow keys would. `draw` records where each one is on screen
//! (`draw::hit_at`); `main.rs` turns a click into a selection plus Enter.

use super::*;

/// Something on screen the mouse can point at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// An item of the title menu or of the pause, game-over or victory popup.
    Menu(usize),
    /// A theme in the theme picker.
    Theme(usize),
    /// A row of the options panel.
    Option(usize),
    /// A shop card, or the reroll / next-wave buttons (by `shop_sel` index).
    Shop(usize),
    /// A row of the upgrades screen.
    Rack(usize),
    /// The upgrades screen's sell button.
    Sell,
    /// Opens the upgrades screen from the shop, or closes it.
    Loadout,
}

impl Game {
    /// The pointer is over `hit`: select it if it belongs to the screen that
    /// is up (with a tick when the selection moves). Returns whether it did,
    /// so a click knows it may press Enter.
    pub fn hover(&mut self, hit: Hit) -> bool {
        let before = (
            self.menu,
            self.theme_choice,
            self.options_sel,
            self.shop_sel,
            self.rack_sel,
        );
        let ok = self.select_hit(hit);
        let after = (
            self.menu,
            self.theme_choice,
            self.options_sel,
            self.shop_sel,
            self.rack_sel,
        );
        if ok && after != before {
            self.sfx(Sfx::Hover);
        }
        ok
    }

    fn select_hit(&mut self, hit: Hit) -> bool {
        let title_menu = self.phase == Phase::Title && !self.options_open && !self.help_open;
        let popup = matches!(self.phase, Phase::Paused | Phase::GameOver | Phase::Victory);
        let shop = self.phase == Phase::Shop;
        match hit {
            Hit::Menu(i) if (title_menu || popup) && !self.theme_menu_open => {
                let ok = i < self.menu_items().len();
                if ok {
                    self.menu = i;
                }
                ok
            }
            Hit::Theme(i) if self.theme_menu_open && i < Theme::ALL.len() => {
                self.theme_choice = i;
                true
            }
            Hit::Option(i) if self.options_open && i < OPTION_ITEMS.len() => {
                self.options_sel = i;
                true
            }
            Hit::Shop(i) if shop && !self.rack_focus && i <= self.shop_go_index() => {
                self.shop_sel = i;
                true
            }
            Hit::Rack(i) if shop && self.rack_focus && i < self.rack_used() as usize => {
                self.rack_sel = i;
                true
            }
            Hit::Sell => shop && self.rack_focus,
            Hit::Loadout => shop,
            _ => false,
        }
    }
}
