//! Trees, grown rather than modelled.
//!
//! **This is a deliberate twin of Opificium's `terrain/tree.rs`.** The two
//! programs share no code — different Bevy majors, separate repositories — so
//! this file exists twice and the copies must stay in step. Everything a tree
//! draws for itself, and the ORDER it draws it in, is load-bearing: reorder two
//! lines of `Habit` and every tree in the pool changes shape, so the bench shows
//! one forest and the game grows another with nothing to say why.
//!
//! `HANDOFF.md` lists exactly what must agree.
//!
//! A tree here is not an asset somebody drew. It is a trunk that tapers, limbs
//! that fork off it at angles the tree itself decides, and leaves at the ends of
//! whatever those limbs turned into. Every tree is grown from one number — its
//! seed — so two trees are alike only if they were given the same one.
//!
//! # A pool of trees, not a tree per tree
//!
//! A forest is tens of thousands of trees and a mesh apiece is not affordable:
//! the memory is the least of it, and the draw calls are the rest. So a modest
//! POOL is grown — a couple of dozen — and the forest plants those, each turned
//! to its own angle and stood at its own height.
//!
//! This is worth being straight about: it is not literally a unique tree per
//! tree. It reads as one, because what the eye picks up at any distance is
//! outline and lean and colour rather than the arrangement of a particular
//! branch, and no two neighbours are the same variant at the same angle. But the
//! honest description is "two dozen trees, planted many times".
//!
//! # Why it is grown at all
//!
//! Because the alternative is asking a maker to draw a hundred trees, and
//! because a grown one can be tuned by the sentence — taller, sparser, more
//! upright — where a drawn one has to be drawn again.

use glam::Vec3;

use crate::{Draw, Geometry};

/// How many distinct trees are grown for a world.
///
/// Enough that a stand of them does not read as a repeating pattern, few enough
/// that they all fit in memory without thought. Raising it costs a mesh each.
pub const VARIETIES: usize = 20;

/// How the tree is put together. Every one of these is a number the tree draws
/// for itself from its seed, within the range given here.
struct Habit {
    /// Metres from root to the top of the trunk.
    height: f32,
    /// Radius at the foot, and the fraction of it left at the crown.
    foot: f32,
    taper: f32,
    /// Sides to the trunk and limbs. Low: this is a forest seen from tens of
    /// metres away, and the silhouette is doing all the work.
    sides: usize,
    /// How many limbs leave the trunk, and how far up it they start.
    limbs: usize,
    limbs_from: f32,
    /// How far a limb leans from vertical, in radians.
    spread: f32,
    /// How much of its parent's length a limb gets.
    limb_length: f32,
    /// How many times a limb forks again.
    forks: usize,
    /// Radius of a leaf cluster at a limb's end.
    leaf: f32,
}

/// A grown tree: trunk and limbs in one mesh, leaves in another.
///
/// Two meshes rather than one because they want different materials — bark is
/// matte and dark, leaves are lighter and want to read as mass rather than
/// surface — and Bevy takes one material per mesh.
pub struct Tree {
    pub wood: Geometry,
    pub leaves: Geometry,
    /// How tall it stands, so the planter can judge what it will cover.
    pub height: f32,
}

/// Grows one tree from a seed.
pub fn grow(seed: u32) -> Tree {
    let mut draw = Draw::new(seed);
    let habit = Habit {
        height: draw.between(7.0, 15.0),
        foot: draw.between(0.16, 0.34),
        taper: draw.between(0.18, 0.42),
        sides: if draw.unit() < 0.5 { 5 } else { 6 },
        limbs: draw.between(3.0, 7.0) as usize,
        limbs_from: draw.between(0.34, 0.55),
        spread: draw.between(0.5, 1.15),
        limb_length: draw.between(0.42, 0.66),
        forks: if draw.unit() < 0.65 { 2 } else { 1 },
        leaf: draw.between(1.1, 2.2),
    };

    let mut wood = Timber::default();
    let mut leaves = Timber::default();

    // The trunk, and then everything that leaves it. `limb` recurses: what it
    // does to the trunk it does to each limb, and to each of theirs, until it
    // runs out of forks and puts leaves on instead.
    let top = Vec3::Y * habit.height;
    wood.branch(
        Vec3::ZERO,
        top,
        habit.foot,
        habit.foot * habit.taper,
        habit.sides,
    );
    limb(
        &mut wood,
        &mut leaves,
        &habit,
        &mut draw,
        Vec3::ZERO,
        top,
        habit.foot * habit.taper,
        habit.forks,
    );

    // A crown at the top of the trunk, so a tree always has leaves over its
    // middle rather than only out on the limbs.
    leaves.blob(top, habit.leaf * 1.15, &mut draw);

    Tree {
        wood: wood.finish(),
        leaves: leaves.finish(),
        height: habit.height,
    }
}

/// Puts limbs on a length of wood, and leaves on the ends of them.
fn limb(
    wood: &mut Timber,
    leaves: &mut Timber,
    habit: &Habit,
    draw: &mut Draw,
    foot: Vec3,
    tip: Vec3,
    girth: f32,
    forks_left: usize,
) {
    let along = tip - foot;
    let length = along.length();

    for i in 0..habit.limbs {
        // Spaced up the parent rather than all from one point, and turned around
        // it as they go, so limbs spiral instead of leaving in a fan.
        let up = habit.limbs_from + (1.0 - habit.limbs_from) * (i as f32 + draw.unit() * 0.6)
            / habit.limbs as f32;
        let from = foot + along * up.min(1.0);

        let turn = draw.unit() * std::f32::consts::TAU;
        let lean = habit.spread * draw.between(0.7, 1.3);
        // Higher limbs lean less: a tree is broad at the bottom and narrow at
        // the top, which is the shape light makes.
        let lean = lean * (1.0 - up * 0.45);

        let out = Vec3::new(turn.cos() * lean.sin(), lean.cos(), turn.sin() * lean.sin());
        let reach = length * habit.limb_length * draw.between(0.75, 1.2) * (1.0 - up * 0.35);
        let end = from + out.normalize() * reach;

        let thin = girth * draw.between(0.42, 0.62);
        wood.branch(from, end, girth * 0.8, thin, habit.sides.max(4) - 1);

        if forks_left > 0 {
            limb(wood, leaves, habit, draw, from, end, thin, forks_left - 1);
        } else {
            // The end of the line: leaves.
            leaves.blob(end, habit.leaf * draw.between(0.75, 1.25), draw);
        }
    }
}

/// A mesh under construction. Thin wrapper over [`Geometry`], because
/// growing a tree is easier to read as `wood.branch(..)` than as index
/// arithmetic in the middle of the shaping.
#[derive(Default)]
struct Timber(Geometry);

impl Timber {
    /// A tapered tube from one point to another: trunk, limb, twig.
    fn branch(&mut self, foot: Vec3, tip: Vec3, wide: f32, narrow: f32, sides: usize) {
        let along = tip - foot;
        if along.length_squared() < 1.0e-6 {
            return;
        }
        let up = along.normalize();
        // Any vector not parallel to the branch will do to get a perpendicular;
        // X unless the branch is pointing along X.
        let aside = if up.x.abs() < 0.9 { Vec3::X } else { Vec3::Z };
        let right = up.cross(aside).normalize();
        let forward = up.cross(right);

        let base = self.0.places.len() as u32;
        for ring in 0..2 {
            let (centre, radius) = if ring == 0 {
                (foot, wide)
            } else {
                (tip, narrow)
            };
            for side in 0..sides {
                let turn = side as f32 / sides as f32 * std::f32::consts::TAU;
                let out = right * turn.cos() + forward * turn.sin();
                let at = centre + out * radius;
                self.0.places.push([at.x, at.y, at.z]);
                self.0.normals.push([out.x, out.y, out.z]);
                self.0.uvs
                    .push([side as f32 / sides as f32, ring as f32]);
            }
        }

        for side in 0..sides {
            let next = (side + 1) % sides;
            let (a, b) = (base + side as u32, base + next as u32);
            let (c, d) = (a + sides as u32, b + sides as u32);
            self.0.indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    /// A rough ball of leaves.
    ///
    /// An octahedron pushed out at every vertex by a different amount, which at
    /// any distance a forest is seen from reads as a clump of foliage and costs
    /// six vertices. A sphere would cost twenty times that to look no better.
    fn blob(&mut self, at: Vec3, radius: f32, draw: &mut Draw) {
        const POINTS: [Vec3; 6] = [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
        ];
        const FACES: [[u32; 3]; 8] = [
            [0, 2, 4],
            [2, 1, 4],
            [1, 3, 4],
            [3, 0, 4],
            [2, 0, 5],
            [1, 2, 5],
            [3, 1, 5],
            [0, 3, 5],
        ];

        let base = self.0.places.len() as u32;
        for point in POINTS {
            // Squashed a little on the vertical, because foliage sits wider than
            // it is tall, and jittered so no two clumps are the same ball.
            let out = point * radius * draw.between(0.72, 1.28) * Vec3::new(1.0, 0.78, 1.0);
            let place = at + out;
            self.0.places.push([place.x, place.y, place.z]);
            let n = out.normalize_or_zero();
            self.0.normals.push([n.x, n.y, n.z]);
            self.0.uvs.push([0.5, 0.5]);
        }
        for face in FACES {
            self.0.indices
                .extend_from_slice(&[base + face[0], base + face[1], base + face[2]]);
        }
    }

    fn finish(self) -> Geometry {
        self.0
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_has_wood_and_leaves() {
        let tree = grow(7);
        assert!(!tree.wood.is_empty(), "a tree needs a trunk");
        assert!(!tree.leaves.is_empty(), "a tree needs leaves");
        assert!((7.0..=15.0).contains(&tree.height), "height {}", tree.height);
    }

    #[test]
    fn the_same_seed_grows_the_same_tree() {
        // The reason this crate exists: both programs grow from the same seeds
        // and never exchange a tree.
        assert_eq!(grow(1234).wood, grow(1234).wood);
        assert_eq!(grow(1234).leaves, grow(1234).leaves);
    }

    #[test]
    fn no_two_trees_in_the_pool_are_the_same_tree() {
        // Vertex COUNT is not a shape — it only moves with limb and fork count,
        // and comparing it once reported five distinct trees out of twenty while
        // they were visibly different. The geometry itself is the claim.
        let shapes: std::collections::HashSet<Vec<u32>> = (0..VARIETIES as u32)
            .map(|seed| {
                grow(seed)
                    .wood
                    .places
                    .iter()
                    .flat_map(|p| p.iter().map(|v| v.to_bits()))
                    .collect()
            })
            .collect();
        assert_eq!(shapes.len(), VARIETIES, "only {} distinct trees", shapes.len());
    }

    #[test]
    fn the_pool_varies_in_outline_not_only_in_detail() {
        // Two trees of a height with limbs in slightly different places are the
        // same tree as far as a forest is concerned.
        let heights: Vec<f32> = (0..VARIETIES as u32).map(|s| grow(s).height).collect();
        let tallest = heights.iter().copied().fold(f32::MIN, f32::max);
        let shortest = heights.iter().copied().fold(f32::MAX, f32::min);
        assert!(tallest - shortest > 4.0, "all one height: {shortest} to {tallest}");
    }

    #[test]
    fn a_tree_stands_on_its_root() {
        // Planted by putting the root at ground level, so anything much below
        // zero is a tree buried to its knees.
        let lowest = grow(99).wood.places.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        assert!(lowest > -0.5, "the trunk starts {lowest:.2} m underground");
    }
}
