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
    /// How far the trunk wanders off vertical over its length, as a fraction of
    /// its height. A tree that grew toward light is not a plumb line.
    sway: f32,
    /// Sides to the trunk and limbs. Low: this is a forest seen from tens of
    /// metres away, and the silhouette is doing all the work.
    sides: usize,
    /// How many limbs leave the trunk, and how far up it they start.
    limbs: usize,
    limbs_from: f32,
    /// How far the LOWEST limbs lean from their parent, in radians.
    ///
    /// From the parent and not from vertical, which is the difference between a
    /// tree and a bundle of sticks: measured against the world, every branch at
    /// every depth re-aims itself upward and the tree grows as a fan of parallel
    /// canes.
    ///
    /// Around ninety degrees is horizontal, and past it a limb hangs below where
    /// it left. Both are ordinary in a real tree and neither was reachable.
    flare: f32,
    /// What fraction of the flare the HIGHEST limbs get.
    ///
    /// A tree is broad at the foot of its crown and narrow at the top, because
    /// that is the shape reaching for light makes. One angle for every limb is a
    /// bottle brush or a starburst and nothing in between — and it was one
    /// angle, scaled only slightly, which is why nothing here had an outline.
    crown_taper: f32,
    /// How much a limb bends along its own length, 0 to 1.
    ///
    /// Small, and it is not always upward. High limbs turn toward the light;
    /// low ones sag under their own weight. Bending every limb skyward — which
    /// is what this did — puts back the very thing the parent frame fixed.
    sweep: f32,
    /// How much of its parent's length a limb gets.
    limb_length: f32,
    /// How many times a limb forks again.
    forks: usize,
    /// Radius of a leaf cluster.
    leaf: f32,
    /// Where this tree sits in the leaf-colour range, 0 to 1. Not geometry —
    /// but two trees the same green are the same tree to the eye however
    /// differently they are built, and one material for a whole forest was
    /// doing more to flatten it than any of the shaping.
    tint: f32,
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
    /// Where this tree sits in the leaf-colour range, 0 to 1. What the planter
    /// does with it is its own business — but it must do SOMETHING, or a wood of
    /// twenty different trees is twenty shapes in one flat green.
    pub tint: f32,
}

/// How many segments a trunk is drawn in.
///
/// One straight tube cannot hold its girth low and thin near the crown, and
/// cannot lean. Six is enough for both and cheap.
const TRUNK_SEGMENTS: usize = 6;

/// Grows one tree from a seed.
pub fn grow(seed: u32) -> Tree {
    let mut draw = Draw::new(seed);

    // Spread is drawn first because so much follows from it. A tree that holds
    // its limbs close carries MORE of them and shorter — a spire; one that
    // throws them wide carries fewer and longer — an oak in a field. Deriving
    // the rest keeps those two from being mixed into one average tree, which is
    // what a pool of independently drawn numbers converges on.
    // How wide the crown opens at its foot. Past ninety degrees the lowest limbs
    // hang below where they left the trunk, which is what an old broad tree
    // does and what nothing here could do.
    let flare = draw.between(0.95, 1.70);
    let openness = (flare - 0.95) / 0.75;

    let height = draw.between(5.0, 18.0);
    let habit = Habit {
        height,
        // Girth from height rather than absolute, so a tall tree is a thick one.
        // A wide range on top of that, because two trees of a height should not
        // be two trees on the same trunk: this spans saplings to old timber.
        foot: height * draw.between(0.017, 0.045),
        // What is LEFT at the crown. Was down to 0.18, so the trunk was a whip
        // for most of the length anyone actually sees.
        taper: draw.between(0.30, 0.52),
        sway: draw.between(0.01, 0.06),
        sides: if draw.unit() < 0.5 { 5 } else { 6 },
        limbs: (7.0 - openness * 3.0).round() as usize,
        limbs_from: draw.between(0.20, 0.44),
        flare,
        crown_taper: draw.between(0.20, 0.46),
        sweep: draw.between(0.08, 0.24),
        limb_length: 0.36 + openness * 0.30,
        forks: if draw.unit() < 0.6 { 2 } else { 1 },
        // Smaller, and there are far more of them. Clusters of 2.2 m radius on a
        // 12 m tree are 4.5 m across — five of those is not foliage, it is five
        // boulders in the sky.
        leaf: height * draw.between(0.05, 0.085),
        tint: draw.unit(),
    };

    let mut wood = Timber::default();
    let mut leaves = Timber::default();

    // The trunk: segment by segment, holding its girth low and leaning as it
    // climbs. Where it ends is where the crown takes over — carrying it to the
    // full height left a bare pole standing out of the leaves.
    let lean = {
        let turn = draw.unit() * std::f32::consts::TAU;
        Vec3::new(turn.cos(), 0.0, turn.sin())
    };
    let trunk_at = |t: f32| Vec3::Y * (habit.height * TRUNK_TOP * t) + lean * (habit.sway * habit.height * t * t);
    let trunk_girth = |t: f32| habit.foot * (1.0 - (1.0 - habit.taper) * t.powf(1.5));

    for segment in 0..TRUNK_SEGMENTS {
        let (low, high) = (
            segment as f32 / TRUNK_SEGMENTS as f32,
            (segment + 1) as f32 / TRUNK_SEGMENTS as f32,
        );
        wood.branch(
            trunk_at(low),
            trunk_at(high),
            trunk_girth(low),
            trunk_girth(high),
            habit.sides,
        );
    }

    let top = trunk_at(1.0);
    limb(
        &mut wood,
        &mut leaves,
        &habit,
        &mut draw,
        trunk_at(0.0),
        top,
        trunk_girth(1.0),
        habit.forks,
        habit.limbs,
        1.0,
    );

    // A crown where the trunk ends, so a tree always has leaves over its middle
    // rather than only out on the limbs.
    leaves.blob(top, habit.leaf * 0.95, &mut draw);

    Tree {
        wood: wood.finish(),
        leaves: leaves.finish(),
        height: habit.height,
        tint: habit.tint,
    }
}

/// Where the trunk stops, as a fraction of the tree's height. The rest is crown.
const TRUNK_TOP: f32 = 0.78;

/// How low foliage may hang, as a fraction of the tree's height.
///
/// Named for what makes it true outdoors: below about here everything gets
/// eaten, so real woods have a clear line under them and a walkable floor.
const BROWSE_LINE: f32 = 0.16;

/// Puts limbs on a length of wood, and leaves on the ends of them.
///
/// Recurses: what it does to the trunk it does to each limb, and to each of
/// theirs, until it runs out of forks and puts leaves on instead.
#[allow(clippy::too_many_arguments)]
fn limb(
    wood: &mut Timber,
    leaves: &mut Timber,
    habit: &Habit,
    draw: &mut Draw,
    foot: Vec3,
    tip: Vec3,
    girth: f32,
    forks_left: usize,
    count: usize,
    // `narrowing` is how much of the full lean these limbs get. Children take
    // less than their parent: a crown divides into finer and finer angles, and
    // children given the parent's own flare swing back past it and tangle.
    narrowing: f32,
) {
    let along = tip - foot;
    let length = along.length();
    if length < 1.0e-4 {
        return;
    }
    let heading = along / length;

    // A frame around the PARENT's direction. Every angle below is measured in
    // this, which is what makes a limb continue the way its parent was going
    // instead of turning back to face the sky.
    let sideways = if heading.y.abs() < 0.95 {
        heading.cross(Vec3::Y).normalize()
    } else {
        heading.cross(Vec3::X).normalize()
    };
    let crossways = heading.cross(sideways);

    for i in 0..count {
        // Spaced up the parent rather than all from one point, and turned around
        // it as they go, so limbs spiral instead of leaving in a fan.
        // Weighted low. Spaced evenly, a crown carries as many limbs in its top
        // quarter as its bottom half, which is not how a tree is built and is
        // half of why the foliage piled up at the top.
        let along_parent = ((i as f32 + draw.unit() * 0.6) / count as f32).powf(0.8);
        let up = habit.limbs_from + (1.0 - habit.limbs_from) * along_parent;
        let from = foot + along * up.min(1.0);

        let turn = draw.unit() * std::f32::consts::TAU;
        // The angle follows WHERE the limb leaves. At the foot of the crown it
        // goes out near horizontal — sometimes past it — and by the top it is
        // only a little off the leader. That interpolation is the crown's
        // outline; one angle for all of them, which is what this was, gives a
        // shape with no silhouette at all.
        //
        // Measured from the foot of the CROWN, not from the root. `up` is where
        // the limb sits on its parent as a whole, which for a tree whose limbs
        // start at 44% means its lowest limb was already treated as two-fifths
        // of the way up — narrow and short before the crown had begun. That is
        // the whole of why four of twenty came out vases.
        let lean =
            habit.flare * (1.0 - (1.0 - habit.crown_taper) * along_parent) * draw.between(0.8, 1.2);
        let lean = lean * narrowing;

        let out = (heading * lean.cos()
            + (sideways * turn.cos() + crossways * turn.sin()) * lean.sin())
        .normalize();

        // The longest limbs are the LOW ones. A tree's widest point is the foot
        // of its crown, and shortening only a third of the way up left the top
        // reaching as far as the bottom — which, with everything also leaning
        // inward up there, is a vase.
        let reach =
            length * habit.limb_length * draw.between(0.75, 1.2) * (1.0 - along_parent * 0.55);
        // Two lengths, not one: a limb that leaves at its angle and holds it
        // dead straight reads as a spoke on a wheel.
        //
        // Which way it bends depends on where it is. High limbs turn toward the
        // light; low ones sag under their own weight and turn DOWN. Bending all
        // of them skyward puts back exactly the fault the parent frame fixed —
        // and it did, which is why every branch still pointed at the sky.
        let elbow = from + out * (reach * 0.55);
        let toward = if along_parent > 0.55 {
            Vec3::Y
        } else {
            Vec3::NEG_Y
        };
        let onward = out.lerp(toward, habit.sweep).normalize_or(out);
        let end = elbow + onward * (reach * 0.45);

        // Nothing hangs into the grass. A limb that leaves near horizontal and
        // then sags can otherwise finish below the ground it grew out of, and a
        // tree with its leaves lying on the turf reads as a bush that has fallen
        // over. Lifted rather than shortened, so the limb keeps its reach.
        let floor = habit.height * BROWSE_LINE;
        let end = Vec3::new(end.x, end.y.max(floor), end.z);
        let elbow = Vec3::new(elbow.x, elbow.y.max(floor * 0.6), elbow.z);

        let thin = girth * draw.between(0.45, 0.66);
        let middling = (girth * 0.82 + thin) * 0.5;
        let ribs = habit.sides.max(4) - 1;
        wood.branch(from, elbow, girth * 0.82, middling, ribs);
        wood.branch(elbow, end, middling, thin, ribs);

        if forks_left > 0 {
            // Fewer sub-limbs than the parent carried, which is both how a tree
            // divides and what keeps the count from cubing itself.
            limb(
                wood,
                leaves,
                habit,
                draw,
                elbow,
                end,
                thin,
                forks_left - 1,
                count.saturating_sub(1).max(2),
                narrowing * 0.6,
            );
        } else {
            // The end of the line. Several small clusters along the last stretch
            // rather than one boulder at the tip: what makes a canopy read as
            // foliage is its edge being broken up, and one blob per limb has
            // almost no edge at all.
            for cluster in 0..3 {
                let at = elbow.lerp(end, 0.45 + cluster as f32 * 0.3);
                let scatter = Vec3::new(
                    draw.between(-0.4, 0.4),
                    draw.between(-0.3, 0.3),
                    draw.between(-0.4, 0.4),
                ) * habit.leaf;
                leaves.blob(at + scatter, habit.leaf * draw.between(0.6, 1.05), draw);
            }
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

    /// Width and height of a tree's foliage, which is the outline anything sees.
    fn crown(tree: &Tree) -> (f32, f32, f32) {
        let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
        for place in &tree.leaves.places {
            for axis in 0..3 {
                low[axis] = low[axis].min(place[axis]);
                high[axis] = high[axis].max(place[axis]);
            }
        }
        let wide = (high[0] - low[0]).max(high[2] - low[2]);
        (wide, high[1] - low[1], low[1])
    }

    #[test]
    fn a_trunk_is_a_trunk_and_not_a_cane() {
        // Every trunk was drawn from an absolute girth of a few centimetres and
        // tapered to a fifth of that over the whole height, so a twelve-metre
        // tree stood on something the thickness of a broom handle. Girth comes
        // from height now, and the taper leaves a third of it at the crown.
        for seed in 0..VARIETIES as u32 {
            let tree = grow(seed);
            let foot: Vec<&[f32; 3]> = tree
                .wood
                .places
                .iter()
                .filter(|place| place[1] < 0.4)
                .collect();
            let across = foot
                .iter()
                .flat_map(|a| foot.iter().map(move |b| (a[0] - b[0]).abs().max((a[2] - b[2]).abs())))
                .fold(0.0, f32::max);

            assert!(
                across > tree.height / 32.0,
                "seed {seed}: a {:.1} m tree on a {:.2} m trunk",
                tree.height,
                across
            );
        }
    }

    #[test]
    fn foliage_hangs_down_the_tree_rather_than_capping_it() {
        // The symptom of limbs aimed at the sky: every branch, at every depth,
        // re-aimed upward and carried its leaves to the top, leaving a bare pole
        // under a lollipop. Leaves should start well down the trunk and cover a
        // real band of it.
        for seed in 0..VARIETIES as u32 {
            let tree = grow(seed);
            let (_, tall, lowest) = crown(&tree);

            assert!(
                lowest < tree.height * 0.72,
                "seed {seed}: the lowest leaf is {:.0}% up a {:.1} m tree",
                lowest / tree.height * 100.0,
                tree.height
            );
            assert!(
                tall > tree.height * 0.3,
                "seed {seed}: foliage only {:.1} m deep on a {:.1} m tree",
                tall,
                tree.height
            );
        }
    }

    #[test]
    fn a_crown_is_about_as_wide_as_it_is_tall() {
        // Limbs leaning from their PARENT rather than from vertical is what puts
        // width on a tree at all. With every branch re-aiming upward the crown
        // came out narrower than its own depth, which is a bottle brush.
        for seed in 0..VARIETIES as u32 {
            let tree = grow(seed);
            let (wide, tall, _) = crown(&tree);
            // Not "wider than deep" — a spire is a real tree and the pool wants
            // some. But half as wide as it is deep is a bottle brush, and a pool
            // with those in it still reads as "these all look wrong".
            assert!(
                wide > tall * 0.55,
                "seed {seed}: crown {wide:.1} m across and {tall:.1} m deep"
            );
        }
    }

    #[test]
    fn a_crown_is_not_a_vase() {
        // The shape of the complaint, measured. Limbs that all point skyward
        // carry their leaves up and outward together, so the foliage comes out
        // pinched at the bottom and widest at the very top — a vase, or a
        // martini glass. A tree is widest through its middle and lower crown,
        // because the limbs down there went out rather than up.
        //
        // This survived the last pass: the limbs left the trunk at the right
        // angle and were then bent back toward Y by the sweep, at every depth.
        for seed in 0..VARIETIES as u32 {
            let tree = grow(seed);
            let (_, tall, lowest) = crown(&tree);

            // How far the foliage stands off the trunk, on AVERAGE, in a band.
            // Not the widest pair in it: a band holding one cluster measures
            // nothing across however far out that cluster is, so a sparse middle
            // reads as a pinched one and the metric answers a question nobody
            // asked.
            let reach_between = |from: f32, to: f32| {
                let (low, high) = (lowest + tall * from, lowest + tall * to);
                let band: Vec<f32> = tree
                    .leaves
                    .places
                    .iter()
                    .filter(|place| place[1] >= low && place[1] <= high)
                    .map(|place| (place[0] * place[0] + place[2] * place[2]).sqrt())
                    .collect();
                if band.is_empty() {
                    return 0.0;
                }
                band.iter().sum::<f32>() / band.len() as f32
            };

            let under = reach_between(0.1, 0.5);
            let over = reach_between(0.7, 1.0);
            assert!(
                under > over * 0.9,
                "seed {seed}: foliage stands {under:.1} m off the trunk low down and \
                 {over:.1} m up top - the limbs are all pointing up"
            );
        }
    }

    #[test]
    fn the_pool_holds_spires_and_spreading_trees_both() {
        // Twenty trees drawn from one set of narrow ranges average into twenty
        // copies of the same tree. Spread is drawn first and the limb count and
        // length follow from it, so the pool keeps its extremes.
        let shapes: Vec<f32> = (0..VARIETIES as u32)
            .map(|seed| {
                let tree = grow(seed);
                let (wide, _, _) = crown(&tree);
                wide / tree.height
            })
            .collect();
        let broadest = shapes.iter().copied().fold(f32::MIN, f32::max);
        let narrowest = shapes.iter().copied().fold(f32::MAX, f32::min);
        assert!(
            broadest - narrowest > 0.3,
            "every tree the same build: {narrowest:.2} to {broadest:.2} wide per metre of height"
        );
    }

    #[test]
    fn no_two_trees_wear_the_same_green() {
        // One leaf material for a whole forest was doing more to flatten it than
        // any of the shaping. The tint is the tree's, so the bench and the game
        // colour the same tree the same way.
        let tints: Vec<f32> = (0..VARIETIES as u32).map(|seed| grow(seed).tint).collect();
        assert!(tints.iter().all(|t| (0.0..=1.0).contains(t)), "{tints:?}");

        let mut sorted = tints.clone();
        sorted.sort_by(f32::total_cmp);
        assert!(
            sorted.windows(2).all(|pair| pair[1] - pair[0] > 1.0e-4),
            "two varieties share a tint: {sorted:?}"
        );
        assert!(
            sorted[VARIETIES - 1] - sorted[0] > 0.6,
            "the whole pool is one shade: {:.2} to {:.2}",
            sorted[0],
            sorted[VARIETIES - 1]
        );
    }

    /// What the pool actually came out as, for tuning by eye.
    ///
    /// `cargo test print_the_pool -- --ignored --nocapture`. The assertions
    /// above say what must never be true again; this says what IS true, which is
    /// the thing you want when the answer is "it still doesn't look right".
    #[test]
    #[ignore = "prints the pool for tuning; not a check"]
    fn print_the_pool() {
        println!(" seed  height   trunk   crown w x d   low/high   lowest leaf   tint");
        for seed in 0..VARIETIES as u32 {
            let tree = grow(seed);
            let (wide, tall, lowest) = crown(&tree);
            let band = |from: f32, to: f32| {
                let (a, b) = (lowest + tall * from, lowest + tall * to);
                let rows: Vec<&[f32; 3]> = tree
                    .leaves
                    .places
                    .iter()
                    .filter(|p| p[1] >= a && p[1] <= b)
                    .collect();
                rows.iter()
                    .flat_map(|a| rows.iter().map(move |b| (a[0] - b[0]).abs().max((a[2] - b[2]).abs())))
                    .fold(0.0_f32, f32::max)
            };
            let (under, over) = (band(0.15, 0.5), band(0.7, 1.0));
            let foot = tree
                .wood
                .places
                .iter()
                .filter(|place| place[1] < 0.2)
                .map(|place| place[0].abs().max(place[2].abs()) * 2.0)
                .fold(0.0, f32::max);
            println!(
                "  {seed:>3}  {:>5.1} m  {:>5.2} m  {:>5.1} x {:>4.1}  {:>4.1}/{:<4.1}  {:>6.0}%   {:>5.2}",
                tree.height,
                foot,
                wide,
                tall,
                under,
                over,
                lowest / tree.height * 100.0,
                tree.tint
            );
        }
    }

    #[test]
    fn a_tree_stands_on_its_root() {
        // Planted by putting the root at ground level, so anything much below
        // zero is a tree buried to its knees.
        let lowest = grow(99).wood.places.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        assert!(lowest > -0.5, "the trunk starts {lowest:.2} m underground");
    }
}
