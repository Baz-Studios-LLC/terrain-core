//! Where the trees are.
//!
//! Two answers, added together.
//!
//! The first comes from the ground itself: trees want moisture, ground below the
//! treeline, a slope they can hold, and somewhere that isn't a beach, a road, or
//! the levelled ground under a town.
//!
//! The second is what a maker painted at Opificium's terrain bench and saved to
//! `assets/world/forest.bin` — signed bias, where **zero leaves the ground's own
//! answer alone**. The game only ever reads it. Nothing here paints, because the
//! game is not where planting happens.
//!
//! # This is a deliberate twin
//!
//! Opificium's `terrain/forest.rs` is the other half. The two programs share no
//! code, so the placement exists twice and the copies must agree **exactly** —
//! the hash multipliers, the six salts and their order, the world-wide slot
//! lattice, and every rejection rule below. A difference of one digit gives the
//! bench one forest and the game another, with no error and nothing failing.
//! `HANDOFF.md` lists the whole contract.
//!
//! No list of trees is ever written down. They scatter from a hash of position,
//! so both programs plant the identical forest without a tree passing between
//! them.

use std::fs;
use std::path::Path;

use glam::{Vec2, Vec3};

use crate::smoothstep;

/// Names the file, so a stale or unrelated one is refused.
const MAGIC: &[u8; 8] = b"RNGRFST1";

/// Meters per cell of the painted layer. Must match the bench's.
pub const CELL: f32 = 16.0;

/// Below this, a cell is untouched and the ground's answer stands.
const PAINTED_EPSILON: f32 = 0.01;

/// One tree, ready to plant.
pub struct Planted {
    pub at: Vec3,
    /// Which of the grown pool this is.
    pub variety: usize,
    /// Turned about its own trunk, so neighbours of one variety don't line up.
    pub turn: f32,
    /// Scaled, so a stand has young trees and old ones in it.
    pub scale: f32,
}

/// The woods a maker painted, read from disk.
pub struct Painted {
    wide: usize,
    deep: usize,
    half: Vec2,
    bias: Vec<f32>,
    painted: usize,
}

impl Painted {
    /// An empty layer: the woods exactly as the ground would have them.
    pub fn empty(half: Vec2) -> Self {
        let wide = (half.x * 2.0 / CELL).ceil() as usize + 1;
        let deep = (half.y * 2.0 / CELL).ceil() as usize + 1;
        Self {
            wide,
            deep,
            half,
            bias: vec![0.0; wide * deep],
            painted: 0,
        }
    }

    pub fn load_from(path: &Path, half: Vec2) -> Self {
        let mut empty = Self::empty(half);
        if !path.exists() {
            // The ordinary case for a world nobody has planted yet. Silent,
            // because it is not news.
            return empty;
        }
        let Ok(bytes) = fs::read(path) else {
            // {}: unreadable - taking the ground's own answer
            return empty;
        };

        let header = 8 + 4 * 4;
        if bytes.len() < header || &bytes[..8] != MAGIC {
            // {} is not a painted forest - ignoring it
            return empty;
        }
        let word = |at: usize| {
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
        };
        let real =
            |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
        let (wide, deep) = (word(8), word(12));
        let saved_half = Vec2::new(real(16), real(20));

        // Refused rather than stretched, the same as the sculpting: woods
        // landing in the wrong places is worse than none, and nothing on screen
        // would say why.
        if wide != empty.wide || deep != empty.deep || saved_half.distance(half) > 1.0 {
            // {} was painted for a {:.0}x{:.0} m world, not this {:.0}x{:.0} m one \
            return empty;
        }
        if bytes.len() < header + wide * deep * 4 {
            // {} is truncated - ignoring it
            return empty;
        }

        empty.bias = (0..wide * deep).map(|i| real(header + i * 4)).collect();
        empty.painted = empty
            .bias
            .iter()
            .filter(|v| v.abs() > PAINTED_EPSILON)
            .count();
        empty
    }

    pub fn painted_cells(&self) -> usize {
        self.painted
    }

    /// The bias at a world position, read between cells.
    pub fn at(&self, x: f32, z: f32) -> f32 {
        let fx = (x + self.half.x) / CELL;
        let fz = (z + self.half.y) / CELL;
        if fx < 0.0 || fz < 0.0 || fx > (self.wide - 1) as f32 || fz > (self.deep - 1) as f32 {
            return 0.0;
        }
        let x0 = fx.floor() as usize;
        let z0 = fz.floor() as usize;
        let x1 = (x0 + 1).min(self.wide - 1);
        let z1 = (z0 + 1).min(self.deep - 1);
        let tx = fx - x0 as f32;
        let tz = fz - z0 as f32;
        let at = |x: usize, z: usize| self.bias[z * self.wide + x];
        let near = at(x0, z0) * (1.0 - tx) + at(x1, z0) * tx;
        let far = at(x0, z1) * (1.0 - tx) + at(x1, z1) * tx;
        near * (1.0 - tz) + far * tz
    }
}

/// What the ground alone says about trees here, 0 to 1.
///
/// Every one of these is a reason a wood would or wouldn't be standing: too dry,
/// too high, too steep, too close to the sea, or ground somebody already
/// levelled to build on.
pub fn natural_density(
    moisture: f32,
    height: f32,
    slope: f32,
    shore: f32,
    levelled: f32,
    treeline: f32,
) -> f32 {
    if shore < 25.0 {
        return 0.0;
    }
    let wet = smoothstep(0.34, 0.62, moisture);
    let low = 1.0 - smoothstep(treeline * 0.72, treeline, height);
    let standable = 1.0 - smoothstep(0.42, 0.72, slope);
    let clear = 1.0 - levelled;
    wet * low * standable * clear
}

/// Combines the ground's answer with what was painted over it.
pub fn density(natural: f32, painted: f32) -> f32 {
    if painted >= 0.0 {
        natural + (1.0 - natural) * painted
    } else {
        natural * (1.0 + painted)
    }
}

/// A repeatable 0..1 from a place and a purpose.
///
/// **Every constant here is part of the contract with Opificium.** Change one
/// and the two forests part company.
pub fn chance(x: i32, z: i32, salt: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(0x8da6_b343)
        .wrapping_add((z as u32).wrapping_mul(0xd8163841))
        .wrapping_add(salt.wrapping_mul(0xcb1a_b31f));
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    h as f32 / u32::MAX as f32
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_grows_in_the_sea_or_on_the_beach() {
        assert_eq!(natural_density(1.0, 2.0, 0.0, -50.0, 0.0, 200.0), 0.0);
        assert_eq!(natural_density(1.0, 2.0, 0.0, 5.0, 0.0, 200.0), 0.0);
    }

    #[test]
    fn woods_want_moisture_and_gentle_ground_below_the_treeline() {
        let good = natural_density(0.9, 40.0, 0.1, 500.0, 0.0, 200.0);
        assert!(good > 0.6, "a wet gentle lowland should be wooded: {good}");
        for (why, thin) in [
            ("dry", natural_density(0.1, 40.0, 0.1, 500.0, 0.0, 200.0)),
            ("high", natural_density(0.9, 205.0, 0.1, 500.0, 0.0, 200.0)),
            ("steep", natural_density(0.9, 40.0, 0.9, 500.0, 0.0, 200.0)),
            ("levelled", natural_density(0.9, 40.0, 0.1, 500.0, 1.0, 200.0)),
        ] {
            assert!(thin < good * 0.35, "{why} ground should be barer: {thin}");
        }
    }

    #[test]
    fn painting_forces_the_question_either_way() {
        assert!(density(0.0, 1.0) > 0.99, "painting should plant bare ground");
        assert!(density(1.0, -1.0) < 0.01, "clearing should empty a wood");
        // Zero is untouched, which is the whole reason it is a bias.
        for natural in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert_eq!(density(natural, 0.0), natural);
        }
    }

    #[test]
    fn neighbouring_slots_do_not_march_in_step() {
        // A scatter that rises in order plants the forest in rows.
        let row: Vec<f32> = (0..12).map(|x| chance(x, 0, 1)).collect();
        let rising = row.windows(2).filter(|w| w[1] > w[0]).count();
        assert!((2..=10).contains(&rising), "the scatter is in order: {row:?}");
    }

    /// The numbers the two programs used to be pinned against each other by.
    ///
    /// Kept as a guard on the crate itself rather than as a contract between the
    /// programs: a change here silently moves every wood in every world already
    /// planted, so it should be a decision and not an accident.
    #[test]
    fn the_scatter_is_what_it_has_always_been() {
        for (x, z, salt, was) in [
            (0, 0, 1u32, 0.427_846_25_f32),
            (0, 0, 3, 0.677_951_81),
            (17, -400, 3, 0.818_481_45),
            (-219, 47, 4, 0.554_404_44),
        ] {
            let now = chance(x, z, salt);
            assert!(
                (now - was).abs() < 1.0e-6,
                "chance({x}, {z}, {salt}) was {was:.8} and is now {now:.8} - \
                 every wood in every planted world just moved"
            );
        }
    }
}
