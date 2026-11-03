//! The four player craft, drawn as braille line art.
//!
//! Every ship is described in square "visual" units (the nose points toward
//! negative y), rotated in that space, then squashed by the cell aspect, so a
//! ship keeps its shape at any heading. Outlines use priority 5, details 4 and
//! soft touches 3.

use super::{ellipse, fade, InkLayer, Palette, CELL_X, CELL_Y};
use crate::game::{rotate, Game, Theme};
use std::f32::consts::TAU;

type Pt = (f32, f32);

/// Draw the ship for the current theme.
pub fn draw_ship(
    ink: &mut InkLayer,
    game: &Game,
    pal: &Palette,
    to_px: impl Fn(f32, f32) -> (f32, f32),
) {
    // Each ship is drawn at the size that keeps its details readable.
    let scale = match game.theme {
        Theme::Space => 1.4,
        Theme::Underwater => 1.35,
        Theme::Jungle => 1.55,
        Theme::CatCafe => 1.5,
    };
    let mut craft = Craft {
        ink,
        to_px,
        at: game.ship_pos(),
        angle: game.angle,
        scale,
    };
    match game.theme {
        Theme::Space => space_ship(&mut craft, game, pal),
        Theme::Underwater => submarine(&mut craft, game, pal),
        Theme::Jungle => monkey(&mut craft, game, pal),
        Theme::CatCafe => cat(&mut craft, game, pal),
    }
}

struct Craft<'a, F> {
    ink: &'a mut InkLayer,
    to_px: F,
    at: Pt,
    angle: f32,
    scale: f32,
}

impl<F: Fn(f32, f32) -> (f32, f32)> Craft<'_, F> {
    fn pt(&self, p: Pt) -> Pt {
        let (x, y) = rotate(p.0 * self.scale, p.1 * self.scale, self.angle);
        (self.to_px)(self.at.0 + x * CELL_X, self.at.1 + y * CELL_Y)
    }

    fn seg(&mut self, a: Pt, b: Pt, pri: u8, col: ratatui::style::Color) {
        let (a, b) = (self.pt(a), self.pt(b));
        self.ink.line(a.0, a.1, b.0, b.1, pri, col);
    }

    fn dot(&mut self, p: Pt, pri: u8, col: ratatui::style::Color) {
        let p = self.pt(p);
        self.ink.dot(p.0, p.1, pri, col);
    }

    fn path(&mut self, pts: &[Pt], closed: bool, pri: u8, col: ratatui::style::Color) {
        let last = if closed { pts.len() } else { pts.len() - 1 };
        for i in 0..last {
            self.seg(pts[i], pts[(i + 1) % pts.len()], pri, col);
        }
    }

    fn ring(&mut self, c: Pt, rx: f32, ry: f32, pri: u8, col: ratatui::style::Color) {
        self.path(&ellipse(c.0, c.1, rx, ry, 16), true, pri, col);
    }

    /// Both halves of a symmetric detail: `pts` and its mirror across x = 0.
    fn pair(&mut self, pts: &[Pt], closed: bool, pri: u8, col: ratatui::style::Color) {
        self.path(pts, closed, pri, col);
        let mirrored: Vec<Pt> = pts.iter().map(|p| (-p.0, p.1)).collect();
        self.path(&mirrored, closed, pri, col);
    }
}

fn flicker(game: &Game) -> f32 {
    0.55 + 0.45 * (game.time * 34.0).sin().abs()
}

/// A swept-wing fighter with a canopy that has googly eyes.
fn space_ship(c: &mut Craft<impl Fn(f32, f32) -> (f32, f32)>, game: &Game, pal: &Palette) {
    let bank = (-game.spin * 0.02).clamp(-0.3, 0.3);
    c.path(
        &[
            (0.0, -2.9),
            (0.62, -1.4),
            (0.78, 0.5),
            (0.58, 1.9),
            (-0.58, 1.9),
            (-0.78, 0.5),
            (-0.62, -1.4),
        ],
        true,
        5,
        pal.ink,
    );
    c.pair(
        &[(0.72, -0.1), (2.1, 1.8), (2.1, 2.4), (0.6, 1.6)],
        false,
        5,
        pal.ink,
    );
    c.pair(&[(2.1, 1.8), (2.1, 2.4)], false, 5, pal.accent);

    // Canopy with eyes that glance the way the ship is turning.
    c.ring((0.0, -0.75), 0.46, 0.62, 4, pal.dim);
    c.dot((-0.2 + bank, -0.78), 5, pal.hot);
    c.dot((0.2 + bank, -0.78), 5, pal.hot);
    c.dot((-0.2 + bank * 1.6, -0.76), 5, pal.ink);
    c.dot((0.2 + bank * 1.6, -0.76), 5, pal.ink);

    // Engine bells and a twin flame.
    c.pair(&[(0.4, 1.9), (0.4, 2.2), (0.15, 2.2)], false, 4, pal.dim);
    let yaw = game.spin.abs() > 1.5;
    if game.thrust > 0.04 || yaw {
        let len = (0.4 + game.thrust * 1.9) * flicker(game);
        c.pair(&[(0.28, 2.25), (0.28, 2.25 + len)], false, 5, pal.accent);
        c.pair(
            &[(0.28, 2.25), (0.4, 2.25 + len * 0.6)],
            false,
            4,
            pal.amber,
        );
    }
    if game.spin.abs() > 1.6 {
        let side = if game.spin > 0.0 { -1.0 } else { 1.0 };
        c.seg((side * 0.8, 0.0), (side * 1.7, 0.5), 5, pal.amber);
    }
}

/// A stubby submarine with a periscope that is also an eyeball.
fn submarine(c: &mut Craft<impl Fn(f32, f32) -> (f32, f32)>, game: &Game, pal: &Palette) {
    let bank = (-game.spin * 0.02).clamp(-0.35, 0.35);
    // Hull: a capsule with a bulb nose.
    let mut hull: Vec<Pt> = Vec::new();
    for i in 0..=8 {
        let a = PI_F * (1.0 + i as f32 / 8.0);
        hull.push((a.cos() * 1.05, -1.6 + a.sin() * 1.25));
    }
    for i in 0..=8 {
        let a = PI_F * i as f32 / 8.0;
        hull.push((a.cos() * 1.05, 1.6 + a.sin() * 1.0));
    }
    c.path(&hull, true, 5, pal.ink);
    c.seg((0.0, -2.7), (0.0, -2.35), 3, pal.dim);

    // Conning tower and a row of portholes.
    c.path(
        &[(-0.5, -0.5), (0.5, -0.5), (0.5, 0.9), (-0.5, 0.9)],
        true,
        5,
        pal.ink,
    );
    c.seg((-0.3, 0.1), (0.3, 0.1), 4, pal.amber);
    for y in [-1.55, 1.45] {
        c.ring((0.0, y), 0.28, 0.28, 4, pal.amber);
        c.dot((0.0, y), 4, pal.hot);
    }

    // The periscope eyeball: it looks where the sub is steering.
    c.seg((0.35, -0.5), (0.95 + bank, -1.15), 4, pal.dim);
    c.ring((1.05 + bank, -1.4), 0.34, 0.34, 5, pal.ink);
    c.dot((1.12 + bank * 2.0, -1.5), 5, pal.hot);

    // Dive planes and stern fins.
    c.pair(&[(1.0, -0.9), (1.75, -0.6), (1.0, -0.3)], false, 4, pal.dim);
    c.pair(&[(0.95, 1.7), (1.8, 2.5), (1.1, 2.55)], false, 4, pal.dim);

    // Propeller.
    let prop = game.time * (7.0 + game.thrust * 18.0);
    let hub = (0.0, 2.85);
    for i in 0..3 {
        let a = prop + i as f32 * TAU / 3.0;
        let tip = (a.cos() * 0.85, hub.1 + a.sin() * 0.4);
        c.seg(hub, tip, 4, fade(pal.accent, 0.75));
        c.dot(tip, 3, pal.accent);
    }
    c.dot(hub, 5, pal.hot);
    if game.spin.abs() > 1.6 {
        let side = if game.spin > 0.0 { -1.0 } else { 1.0 };
        c.seg((side * 1.2, 2.2), (side * 2.0, 2.9), 5, pal.amber);
    }
}

const PI_F: f32 = std::f32::consts::PI;

/// A flight-goggled monkey, winding up a banana throw.
fn monkey(c: &mut Craft<impl Fn(f32, f32) -> (f32, f32)>, game: &Game, pal: &Palette) {
    let wag = (game.time * 2.6).sin() * 0.3 + game.spin * 0.04;
    // Tail curls out behind.
    c.path(
        &[
            (0.1, 1.9),
            (0.6 + wag * 0.5, 2.5),
            (1.4 + wag, 2.7),
            (1.9 + wag, 2.2),
            (1.7 + wag, 1.7),
        ],
        false,
        4,
        pal.dim,
    );
    // Legs and feet.
    c.pair(&[(0.45, 1.8), (0.6, 2.3), (1.0, 2.35)], false, 5, pal.ink);

    // Body and head.
    c.ring((0.0, 1.0), 0.85, 0.95, 5, pal.ink);
    c.ring((0.0, -0.9), 1.05, 1.0, 5, pal.ink);
    c.ring((-1.25, -1.0), 0.4, 0.42, 5, pal.ink);
    c.ring((1.25, -1.0), 0.4, 0.42, 5, pal.ink);

    // Muzzle and grin.
    c.ring((0.0, -0.5), 0.55, 0.42, 4, pal.dim);
    c.path(
        &[(-0.32, -0.36), (0.0, -0.22), (0.32, -0.36)],
        false,
        4,
        pal.dim,
    );

    // Aviator goggles.
    c.ring((-0.45, -1.15), 0.32, 0.32, 5, pal.amber);
    c.ring((0.45, -1.15), 0.32, 0.32, 5, pal.amber);
    c.dot((-0.45, -1.15), 5, pal.hot);
    c.dot((0.45, -1.15), 5, pal.hot);
    c.seg((-0.13, -1.15), (0.13, -1.15), 4, pal.amber);

    // Hair tuft.
    c.path(
        &[(-0.2, -2.35), (0.0, -1.95), (0.0, -2.5)],
        false,
        3,
        pal.ink,
    );
    c.seg((0.0, -1.95), (0.2, -2.35), 3, pal.ink);

    // Left arm holds a banana.
    c.path(&[(-0.75, 0.5), (-1.3, 0.95)], false, 4, pal.ink);
    c.path(
        &[(-1.55, 0.55), (-1.75, 1.0), (-1.45, 1.4), (-1.1, 1.45)],
        false,
        4,
        pal.amber,
    );
    c.dot((-1.1, 1.45), 4, pal.hot);

    // Right arm winds up and throws when the player fires.
    let throw = game.thrust.clamp(0.0, 1.0);
    let elbow = (1.35, 0.55 - 0.5 * throw);
    let hand = (1.65 + 0.55 * throw, 1.0 - 2.4 * throw);
    c.seg((0.75, 0.5), elbow, 4, pal.ink);
    c.seg(elbow, hand, 4, pal.ink);
    c.dot(hand, 5, pal.ink);
}

/// A barista cat in a bow tie, with claws out when it pounces.
fn cat(c: &mut Craft<impl Fn(f32, f32) -> (f32, f32)>, game: &Game, pal: &Palette) {
    let tilt = (-game.spin * 0.03).clamp(-0.4, 0.4);
    let wag = (game.time * 3.2).sin() * 0.45 + game.spin * 0.05;

    // Tail swishing behind.
    c.path(
        &[
            (0.5, 2.3),
            (1.2 + wag * 0.4, 2.8),
            (1.8 + wag * 0.8, 2.6),
            (2.0 + wag, 2.0),
        ],
        false,
        4,
        pal.dim,
    );
    c.dot((2.0 + wag, 2.0), 4, pal.amber);

    // Body and hind paws.
    c.ring((0.0, 1.55), 0.95, 1.05, 5, pal.ink);
    c.pair(&[(0.4, 2.55), (0.6, 2.75), (1.0, 2.75)], false, 5, pal.ink);

    // Bow tie.
    c.path(
        &[
            (0.0, 0.5),
            (-0.5, 0.25),
            (-0.5, 0.75),
            (0.0, 0.5),
            (0.5, 0.25),
            (0.5, 0.75),
        ],
        true,
        4,
        pal.danger,
    );

    // Left paw rests, right paw swipes.
    c.dot((-0.85, 1.25), 5, pal.ink);
    let swipe = game.thrust.clamp(0.0, 1.0);
    let paw = (0.9 + 0.55 * swipe, 1.1 - 2.1 * swipe);
    c.seg((0.6, 1.0), paw, 4, pal.ink);
    c.dot(paw, 5, if swipe > 0.3 { pal.amber } else { pal.ink });
    if swipe > 0.3 {
        for dx in [-0.22, 0.0, 0.22] {
            c.seg(paw, (paw.0 + dx, paw.1 - 0.4), 5, pal.hot);
        }
    }

    // Wide head with ears.
    c.ring((tilt, -0.6), 1.2, 1.0, 5, pal.ink);
    let ear = |s: f32| {
        [
            (s * 1.1 + tilt, -1.2),
            (s * 0.95 + tilt, -2.35),
            (s * 0.25 + tilt, -1.55),
        ]
    };
    c.path(&ear(-1.0), false, 5, pal.ink);
    c.path(&ear(1.0), false, 5, pal.ink);
    c.pair(
        &[(0.8 + tilt, -1.45), (0.78 + tilt, -2.0)],
        false,
        3,
        pal.amber,
    );

    // Big eyes with slit pupils, blush and a "w" mouth.
    for s in [-1.0, 1.0] {
        c.ring((s * 0.5 + tilt, -0.8), 0.3, 0.34, 5, pal.amber);
        c.seg((s * 0.5 + tilt, -1.0), (s * 0.5 + tilt, -0.6), 5, pal.hot);
    }
    c.dot((tilt, -0.5), 5, pal.danger);
    c.path(
        &[
            (-0.3 + tilt, -0.3),
            (-0.15 + tilt, -0.12),
            (tilt, -0.3),
            (0.15 + tilt, -0.12),
            (0.3 + tilt, -0.3),
        ],
        false,
        4,
        pal.dim,
    );

    // Whiskers.
    for s in [-1.0, 1.0] {
        c.seg((s * 1.15 + tilt, -0.6), (s * 1.9 + tilt, -0.75), 3, pal.dim);
        c.seg((s * 1.1 + tilt, -0.3), (s * 1.85 + tilt, -0.1), 3, pal.dim);
    }
}
