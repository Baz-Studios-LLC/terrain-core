//! The world generation Baz Studios games and Opificium's terrain bench both run.
//!
//! # Why this crate exists
//!
//! It was written twice, and the two copies had to agree exactly. A game and the
//! bench that shapes its ground both work the world out from scratch — nothing
//! but files pass between them — so a difference of one digit in a hash, or two
//! lines of a tree's own shaping swapped, gave the bench one world and the game
//! another. No error. Nothing failing. Just wrong.
//!
//! That was held together by tests pinning literal numbers copied out of one
//! program and asserted in the other. It worked, and it was a tax on every
//! change. Written once, the two cannot disagree at all.
//!
//! This is how the studios do it: an editor is built ON TOP of the game's own
//! runtime rather than beside it, and the world code exists once. Ours are
//! separate applications, so the shared part is a crate instead of a module —
//! but the principle is the same, and the alternative is what we had.
//!
//! # It names no engine
//!
//! Nothing here mentions Bevy. It cannot: the game and the bench are on
//! different Bevy majors and could not link the same one. It does not need to
//! either — Bevy's `Vec2` and `Vec3` are `glam`'s, re-exported, and everything
//! here is arithmetic over vectors.
//!
//! Geometry comes out as plain vertex arrays ([`Geometry`]), and each program
//! turns those into its own engine's mesh. That seam is a dozen lines on each
//! side and is the only engine-shaped thing in the whole arrangement.

pub mod biome;
pub mod cloud;
pub mod cover;
pub mod forest;
mod history;
pub mod painted;
pub mod sculpt;
pub mod tree;

pub use glam::{Vec2, Vec3};

/// A patch of ground that changed, as a pair of corners: low, then high.
///
/// Not a rectangle type. Every engine has one and this crate names none, so the
/// two corners cross the boundary and each program turns them into whatever its
/// own is called — one line, at the one place that needs it.
pub type Patch = (Vec2, Vec2);

/// Hermite smoothstep: 0 below `edge0`, 1 above `edge1`, eased between.
///
/// Used everywhere a thing becomes another thing. Shared rather than copied
/// because a subtly different easing curve on either side would move every
/// coastline, every beach and every treeline by a little.
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < f32::EPSILON {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A mesh, before any engine has an opinion about it.
///
/// Positions, normals and texture coordinates as plain arrays, with indices into
/// them. Both programs build their own engine's mesh from this — the one seam
/// where the shared world meets a particular renderer.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct Geometry {
    pub places: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// Linear RGBA per vertex, or empty when the mesh has none.
    ///
    /// Optional because most geometry here does not want it: a tree is coloured
    /// by the material its variety wears, which is cheaper and lets one mesh be
    /// planted a thousand times in a thousand shades. Ground cover cannot do
    /// that — a chunk's worth of grass is welded into ONE mesh, and one mesh
    /// wears one material, so a meadow's many greens have to live in its
    /// vertices.
    pub colours: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl Geometry {
    pub fn vertices(&self) -> usize {
        self.places.len()
    }

    pub fn is_empty(&self) -> bool {
        self.places.is_empty()
    }
}

/// A repeatable stream of numbers from one seed.
///
/// Hashed rather than drawn from a generator crate, so that a given seed gives
/// the same answer in both programs whatever else either happens to be asking
/// for numbers at the time. Every constant here is load-bearing.
pub struct Draw {
    state: u32,
}

impl Draw {
    pub fn new(seed: u32) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9,
        }
    }

    pub fn unit(&mut self) -> f32 {
        let mut h = self.state;
        h ^= h >> 16;
        h = h.wrapping_mul(0x7feb_352d);
        h ^= h >> 15;
        h = h.wrapping_mul(0x846c_a68b);
        h ^= h >> 16;
        self.state = self.state.wrapping_add(0x9E37_79B9);
        h as f32 / u32::MAX as f32
    }

    pub fn between(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothstep_eases_between_its_edges() {
        assert_eq!(smoothstep(0.0, 1.0, -1.0), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 2.0), 1.0);
        assert_eq!(smoothstep(0.0, 1.0, 0.5), 0.5);
        // Backwards edges invert, which several callers rely on to fade OUT.
        assert_eq!(smoothstep(1.0, 0.0, 0.0), 1.0);
        // Equal edges must not divide by zero.
        assert_eq!(smoothstep(1.0, 1.0, 0.5), 0.0);
        assert_eq!(smoothstep(1.0, 1.0, 1.5), 1.0);
    }

    #[test]
    fn a_seed_always_draws_the_same_numbers() {
        let drawn: Vec<f32> = (0..6).map(|_| Draw::new(42).unit()).collect();
        assert!(drawn.windows(2).all(|w| w[0] == w[1]), "same seed, same first draw");

        let mut draw = Draw::new(42);
        let run: Vec<f32> = (0..8).map(|_| draw.unit()).collect();
        assert!(run.iter().all(|v| (0.0..=1.0).contains(v)), "outside 0..1: {run:?}");
        // A stream that repeats itself would give every tree the same limb.
        assert!(run.windows(2).all(|w| w[0] != w[1]), "the stream stalled: {run:?}");
    }
}
