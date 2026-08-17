//! Where the trees are.
//!
//! Two answers, added together.
//!
//! The first comes from the ground itself: trees want moisture, ground below the
//! treeline, a slope they can hold, and somewhere that isn't a beach, a road, or
//! the levelled ground under a town.
//!
//! The second is what a maker painted — signed bias, where **zero leaves the
//! ground's own answer alone**, saved as `forest.bin` beside the world.
//!
//! # This was a deliberate twin, and that is why it is here
//!
//! The placement existed twice, once in a game and once in Opificium's terrain
//! bench, and the copies had to agree **exactly**: the hash multipliers, the six
//! salts and their order, the world-wide slot lattice, and every rejection rule
//! below. A difference of one digit gave the bench one forest and the game
//! another — no error, nothing failing. It was held together by tests pinning
//! literal numbers copied from one program into the other.
//!
//! Written once, they cannot disagree at all. The constants are still
//! load-bearing — changing [`chance`] moves every wood in every world already
//! planted — but they are guarded against ACCIDENT now rather than against a
//! second implementation.
//!
//! No list of trees is ever written down. They scatter from a hash of position,
//! so both programs plant the identical forest without a tree passing between
//! them.


use glam::{Vec2, Vec3};

use crate::smoothstep;

pub use crate::painted::{Kind, Painted};
pub use crate::Patch;

/// Meters per cell of the woods layer.
pub const CELL: f32 = Kind::Woods.cell();

/// An empty woods layer, for a world nobody has planted.
pub fn empty(half: Vec2) -> Painted {
    Painted::empty(Kind::Woods, half)
}

/// Reads planted woods from the bytes of a `forest.bin`.
pub fn read(bytes: &[u8], half: Vec2) -> Result<Painted, String> {
    Painted::read(bytes, Kind::Woods, half)
}

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
            (0, 0, 3, 0.677_951_8),
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
