//! The things a landscape has lying about in it.
//!
//! Boulders, scree, bushes, stumps, fallen logs, dead standing trees, cactus and
//! dry brush. Grown the same way trees are and for the same reasons — a pool of
//! a couple of dozen, scattered from a hash of position, so nothing is stored
//! anywhere and both this game and Opificium's bench lay out the identical world.
//!
//! # What they are for
//!
//! A landscape of ground and trees reads as a golf course. What tells you where
//! you are is the litter: a field with stones in it is farmland, the same field
//! with stumps and fallen wood is a clearing somebody logged, the same field with
//! nothing but dry brush is somewhere nothing much lives. Every kind here is
//! keyed to a biome for that reason, and later for the other one — a monster that
//! lives under rocks needs rocks to live under.
//!
//! # One mesh, one material, colour in the vertices
//!
//! Unlike a tree, which wears its bark and its leaves as two materials, a prop
//! carries its colour in its own vertices. There are eight kinds of thing here
//! and they are made of stone, wood, leaf and dead grass — eight materials for
//! objects this small is worse value than eight sets of vertex colours, and one
//! material for the lot means the whole scatter draws in one pass.

use glam::Vec3;

use crate::biome::Biome;
use crate::timber::Timber;
use crate::{Draw, Geometry};

/// A kind of natural object.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    /// A stone too big to move, sunk into the ground it sits on.
    Boulder,
    /// A spill of small broken stone. What a slope sheds.
    Scree,
    /// Low woody growth. The layer between the grass and the trees.
    Bush,
    /// What is left where a tree came down.
    Stump,
    /// And the tree that came down, lying beside it.
    Log,
    /// A dead tree still standing: bare, broken-limbed, pale.
    Snag,
    /// Dry country's answer to a tree.
    Cactus,
    /// A tangle of dead sticks, which is most of what dry ground grows.
    Brush,
}

impl Kind {
    pub const ALL: [Kind; 8] = [
        Kind::Boulder,
        Kind::Scree,
        Kind::Bush,
        Kind::Stump,
        Kind::Log,
        Kind::Snag,
        Kind::Cactus,
        Kind::Brush,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Boulder => "boulder",
            Kind::Scree => "scree",
            Kind::Bush => "bush",
            Kind::Stump => "stump",
            Kind::Log => "log",
            Kind::Snag => "snag",
            Kind::Cactus => "cactus",
            Kind::Brush => "brush",
        }
    }

    fn place(self) -> usize {
        Kind::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }
}

/// How many of each kind are grown.
///
/// Fewer than a tree gets, and that is the right trade. A prop is a metre of
/// object seen at ground level among hundreds of others, so what separates two
/// of them is which way round they were turned far more than how they were
/// built — where a tree is a silhouette against the sky and repetition in one
/// shows up across a whole hillside.
pub const VARIANTS: usize = 3;

/// Everything in the pool.
pub const VARIETIES: usize = Kind::ALL.len() * VARIANTS;

/// A grown object, ready to be stood on the ground.
pub struct Prop {
    pub mesh: Geometry,
    pub kind: Kind,
    /// How far it reaches from its own middle, in metres, so a planter can
    /// judge what it will cover and how far off the next one should be.
    pub reach: f32,
}

/// Which kinds belong in a place, in the order they are drawn for.
///
/// Empty means nothing of this sort grows here at all: open water has no litter,
/// and what lies about in a town is somebody's business rather than the wild's.
fn belongs(biome: Biome) -> &'static [Kind] {
    match biome {
        Biome::Water | Biome::Settled => &[],
        // Stones in a field, scrub at the edges. Thin, because open grassland
        // that is not open is not grassland.
        Biome::Grass => &[Kind::Boulder, Kind::Bush, Kind::Brush],
        // The full floor of a wood: what fell, what is rotting, what grew in
        // the gap it left.
        Biome::Forest => &[
            Kind::Bush,
            Kind::Log,
            Kind::Stump,
            Kind::Boulder,
            Kind::Snag,
        ],
        // Above the trees it is stone and the last dead ones that tried.
        Biome::Rock => &[Kind::Scree, Kind::Boulder, Kind::Snag],
        Biome::Snow => &[Kind::Boulder, Kind::Scree],
        Biome::Desert => &[Kind::Cactus, Kind::Brush, Kind::Boulder],
        // Driftwood and stones. Whatever the sea put there.
        Biome::Shore => &[Kind::Log, Kind::Boulder, Kind::Brush],
    }
}

/// How thickly things lie in a place, 0 none to 1 as thick as it gets.
///
/// Bare rock sheds stone constantly and a wood is a floor of its own wreckage,
/// where grassland is grassland precisely because it is open. This is the number
/// that decides whether a landscape reads as littered or as swept.
pub fn density(biome: Biome) -> f32 {
    match biome {
        Biome::Water | Biome::Settled => 0.0,
        Biome::Grass => 0.14,
        Biome::Forest => 0.5,
        Biome::Rock => 0.62,
        Biome::Snow => 0.2,
        Biome::Desert => 0.3,
        Biome::Shore => 0.12,
    }
}

/// Which of the pool belongs here, or nothing.
///
/// Two rolls: one picks the KIND from what belongs in this biome, the other
/// picks which of that kind's variants. Split so that a place with three kinds
/// in it gets all three mixed rather than one kind for a whole hillside.
pub fn pick(biome: Biome, kind_roll: f32, variant_roll: f32) -> Option<usize> {
    let choices = belongs(biome);
    if choices.is_empty() {
        return None;
    }
    let kind = choices[pick_one(kind_roll, choices.len())];
    Some(kind.place() * VARIANTS + pick_one(variant_roll, VARIANTS))
}

/// Turns a 0..1 roll into one of `count`, without ever landing on `count`.
fn pick_one(roll: f32, count: usize) -> usize {
    ((roll.clamp(0.0, 1.0) * count as f32) as usize).min(count - 1)
}

/// Grows one of the pool by its index.
pub fn from_pool(index: usize) -> Prop {
    let index = index.min(VARIETIES - 1);
    // Stature comes from WHICH variant this is rather than from the draw. Three
    // samples of a random range land wherever they land — the three snags came
    // out 2.7, 2.9 and 3.0 metres, which is one snag grown three times — so the
    // variants are spread across the range on purpose and the draw is left to
    // decide everything else about them.
    let stature = (index % VARIANTS) as f32 / (VARIANTS - 1).max(1) as f32;
    grow_as(Kind::ALL[index / VARIANTS], index as u32, stature)
}

/// Grows one of a given kind from a seed, at a given stature: 0 the smallest of
/// its sort, 1 the largest.
pub fn grow_as(kind: Kind, seed: u32, stature: f32) -> Prop {
    // Offset so that variant 0 of the first kind is not the same draw as
    // variant 0 of the next — the pool is walked in order, and a shared seed
    // would give every kind the same "small, leaning left" first variant.
    let mut draw = Draw::new(seed.wrapping_mul(0x9E37_79B9).wrapping_add(kind.place() as u32));
    let mut it = Timber::default();
    let grown = Grown {
        stature: stature.clamp(0.0, 1.0),
    };

    let reach = match kind {
        Kind::Boulder => boulder(&mut it, &mut draw, grown),
        Kind::Scree => scree(&mut it, &mut draw, grown),
        Kind::Bush => bush(&mut it, &mut draw, grown),
        Kind::Stump => stump(&mut it, &mut draw, grown),
        Kind::Log => log(&mut it, &mut draw, grown),
        Kind::Snag => snag(&mut it, &mut draw, grown),
        Kind::Cactus => cactus(&mut it, &mut draw, grown),
        Kind::Brush => brush(&mut it, &mut draw, grown),
    };

    Prop {
        mesh: it.finish(),
        kind,
        reach,
    }
}

/// How big this variant of its kind is meant to come out.
#[derive(Clone, Copy)]
struct Grown {
    stature: f32,
}

impl Grown {
    /// A size somewhere in a range, mostly decided by the variant and a little
    /// by the draw — so the three of a kind are reliably a small, a middling
    /// and a large one, and are still not identical within that.
    fn size(self, draw: &mut Draw, low: f32, high: f32) -> f32 {
        let along = self.stature * 0.78 + draw.between(0.0, 1.0) * 0.22;
        low + (high - low) * along
    }
}

// ------------------------------------------------------------------ the shapes

/// A stone, sunk into the ground.
///
/// Two or three lumps overlapping rather than one, because a single lump is an
/// egg. **Sunk** is the important part: a boulder resting exactly on the surface
/// reads as a ball somebody put down, and one with its bottom third underground
/// reads as something that has been there longer than you have.
fn boulder(it: &mut Timber, draw: &mut Draw, grown: Grown) -> f32 {
    let size = grown.size(draw, 0.55, 1.7);
    it.colour = Some(stone(draw));

    let lumps = draw.between(2.0, 3.4) as usize;
    for lump in 0..lumps {
        let out = Vec3::new(
            draw.between(-0.5, 0.5),
            draw.between(-0.25, 0.15),
            draw.between(-0.5, 0.5),
        ) * size;
        let radius = size * draw.between(0.55, 1.0) / (1.0 + lump as f32 * 0.25);
        it.blob(out + Vec3::new(0.0, size * 0.45, 0.0), radius, draw);
    }
    size * 1.4
}

/// A spill of broken stone, which is what a slope does over time.
fn scree(it: &mut Timber, draw: &mut Draw, grown: Grown) -> f32 {
    let spread = grown.size(draw, 1.2, 2.6);
    it.colour = Some(stone(draw));

    let stones = draw.between(5.0, 9.4) as usize;
    for _ in 0..stones {
        let out = Vec3::new(draw.between(-1.0, 1.0), 0.0, draw.between(-1.0, 1.0)) * spread;
        let radius = draw.between(0.10, 0.28);
        // Barely proud of the ground. Scree is a surface, not a heap — a pile of
        // balls is a cairn, and somebody built that.
        it.blob(out + Vec3::new(0.0, radius * 0.35, 0.0), radius, draw);
    }
    spread * 1.1
}

/// Low woody growth: the layer a wood has between its floor and its canopy.
fn bush(it: &mut Timber, draw: &mut Draw, grown: Grown) -> f32 {
    let size = grown.size(draw, 0.45, 1.1);
    let green = leaf(draw);

    // A couple of stems, so it grows out of the ground rather than hovering.
    it.colour = Some(BRANCH);
    let stems = draw.between(2.0, 3.4) as usize;
    for _ in 0..stems {
        let lean = Vec3::new(draw.between(-0.3, 0.3), 1.0, draw.between(-0.3, 0.3)).normalize();
        it.tube(
            &[
                (Vec3::ZERO, size * 0.05),
                (lean * size * 0.7, size * 0.03),
            ],
            4,
            false,
        );
    }

    it.colour = Some(green);
    let clumps = draw.between(4.0, 7.4) as usize;
    for clump in 0..clumps {
        // Piled up rather than spread flat, and thinning toward the top, which
        // is the difference between a bush and a doormat.
        let up = (clump as f32 + 0.5) / clumps as f32;
        let out = Vec3::new(
            draw.between(-1.0, 1.0) * (1.0 - up * 0.6),
            up * 0.9,
            draw.between(-1.0, 1.0) * (1.0 - up * 0.6),
        ) * size;
        it.blob(out + Vec3::new(0.0, size * 0.35, 0.0), size * draw.between(0.4, 0.62), draw);
    }
    size * 1.5
}

/// What is left where a tree came down.
fn stump(it: &mut Timber, draw: &mut Draw, grown: Grown) -> f32 {
    let girth = grown.size(draw, 0.22, 0.42);
    let tall = draw.between(0.35, 0.8);
    let lean = Vec3::new(draw.between(-0.1, 0.1), 1.0, draw.between(-0.1, 0.1)).normalize();

    it.colour = Some(BARK);
    it.tube(
        &[
            // Started below the ground, so no cut edge shows on a slope.
            (lean * -0.3, girth * 1.25),
            (Vec3::ZERO, girth * 1.15),
            (lean * tall, girth),
        ],
        7,
        true,
    );

    // The break itself, in pale wood. This is the whole tell: a brown cylinder
    // is a post, and a brown cylinder with a raw top is a tree that broke.
    it.colour = Some(HEARTWOOD);
    it.tube(
        &[(lean * tall, girth * 0.98), (lean * (tall + 0.04), girth * 0.9)],
        7,
        true,
    );
    girth * 1.4
}

/// A fallen tree, lying where it fell.
fn log(it: &mut Timber, draw: &mut Draw, grown: Grown) -> f32 {
    let girth = draw.between(0.16, 0.34);
    let long = grown.size(draw, 1.8, 4.2);
    let turn = draw.between(0.0, std::f32::consts::TAU);
    let along = Vec3::new(turn.cos(), 0.0, turn.sin());
    // Lying ON the ground, so its middle is a radius up and its ends sag to it.
    let rest = Vec3::new(0.0, girth * 0.8, 0.0);

    it.colour = Some(BARK);
    it.tube(
        &[
            (rest - along * long * 0.5, girth * 0.82),
            (rest - along * long * 0.15, girth),
            (rest + along * long * 0.25, girth * 0.9),
            (rest + along * long * 0.5, girth * 0.7),
        ],
        7,
        true,
    );

    // A stub or two of broken branch, which is what stops it being a pipe.
    it.colour = Some(BRANCH);
    let stubs = draw.between(1.0, 2.4) as usize;
    for _ in 0..stubs {
        let at = rest + along * draw.between(-0.35, 0.35) * long;
        let out = Vec3::new(draw.between(-1.0, 1.0), draw.between(0.2, 1.0), draw.between(-1.0, 1.0))
            .normalize();
        it.tube(
            &[(at, girth * 0.3), (at + out * draw.between(0.25, 0.6), girth * 0.14)],
            5,
            true,
        );
    }
    long * 0.6
}

/// A dead tree still standing.
///
/// Bare, pale and broken-topped. One of these on a ridge does more for a skyline
/// than another live tree does, because it is the one shape up there that is not
/// a cone.
fn snag(it: &mut Timber, draw: &mut Draw, grown: Grown) -> f32 {
    let girth = draw.between(0.16, 0.30);
    let tall = grown.size(draw, 2.6, 6.0);
    let lean = Vec3::new(draw.between(-0.22, 0.22), 1.0, draw.between(-0.22, 0.22)).normalize();

    it.colour = Some(DEADWOOD);
    it.tube(
        &[
            (lean * -0.3, girth * 1.3),
            (lean * tall * 0.35, girth * 0.8),
            (lean * tall * 0.72, girth * 0.55),
            // Snapped off rather than tapering to a point. A dead tree does not
            // keep its top.
            (lean * tall, girth * 0.42),
        ],
        6,
        true,
    );

    let limbs = draw.between(2.0, 4.4) as usize;
    for limb in 0..limbs {
        let up = 0.35 + (limb as f32 / limbs as f32) * 0.5;
        let at = lean * tall * up;
        let turn = draw.between(0.0, std::f32::consts::TAU);
        // Dead limbs droop; nothing is holding them up any more.
        let out = Vec3::new(turn.cos(), draw.between(-0.5, 0.1), turn.sin()).normalize();
        let reach = draw.between(0.5, 1.5);
        it.tube(
            &[
                (at, girth * 0.4),
                (at + out * reach * 0.6, girth * 0.22),
                (at + out * reach, girth * 0.1),
            ],
            5,
            true,
        );
    }
    tall * 0.35
}

/// Dry country's answer to a tree.
fn cactus(it: &mut Timber, draw: &mut Draw, grown: Grown) -> f32 {
    let girth = grown.size(draw, 0.16, 0.28);
    let tall = grown.size(draw, 1.3, 2.9);

    it.colour = Some(SUCCULENT);
    it.tube(
        &[
            (Vec3::new(0.0, -0.2, 0.0), girth),
            (Vec3::new(0.0, tall * 0.5, 0.0), girth * 1.02),
            (Vec3::new(0.0, tall, 0.0), girth * 0.86),
        ],
        8,
        true,
    );

    // Arms: out, then a right angle up. That elbow is the whole silhouette —
    // an arm that curves away reads as a branch and the thing stops being a
    // cactus.
    let arms = draw.between(1.0, 2.6) as usize;
    for arm in 0..arms {
        let turn = draw.between(0.0, std::f32::consts::TAU) + arm as f32 * 2.4;
        let out = Vec3::new(turn.cos(), 0.0, turn.sin());
        let from = Vec3::new(0.0, tall * draw.between(0.3, 0.55), 0.0);
        let elbow = from + out * draw.between(0.3, 0.55) + Vec3::Y * 0.12;
        let up = elbow + Vec3::Y * draw.between(0.4, 1.0);
        it.tube(
            &[
                (from, girth * 0.66),
                (elbow, girth * 0.6),
                (up, girth * 0.52),
            ],
            7,
            true,
        );
    }
    girth * 4.0
}

/// A tangle of dead sticks, which is most of what dry ground grows.
fn brush(it: &mut Timber, draw: &mut Draw, grown: Grown) -> f32 {
    let size = grown.size(draw, 0.35, 0.8);
    it.colour = Some(DRY);

    let sticks = draw.between(5.0, 9.4) as usize;
    for _ in 0..sticks {
        let turn = draw.between(0.0, std::f32::consts::TAU);
        // Splayed outward and up, all from one root, which is what a dead shrub
        // collapses into.
        let out = Vec3::new(turn.cos() * draw.between(0.4, 1.0), draw.between(0.5, 1.3), turn.sin() * draw.between(0.4, 1.0))
            .normalize();
        let reach = size * draw.between(0.7, 1.3);
        it.tube(
            &[
                (Vec3::new(0.0, 0.02, 0.0), size * 0.05),
                (out * reach * 0.55, size * 0.035),
                (out * reach, size * 0.015),
            ],
            4,
            false,
        );
    }
    size * 1.2
}

// ----------------------------------------------------------------- the palette
//
// Linear, because that is what a vertex colour is taken as, and baked in here
// rather than left to the caller for the same reason the ground cover's is:
// there is one mesh and one material for the whole scatter, so colour has
// nowhere else to live.

/// Stone, drawn somewhere between wet slate and dry granite.
///
/// Varied per object rather than per kind. A field of boulders in one grey is a
/// field of one boulder.
fn stone(draw: &mut Draw) -> [f32; 4] {
    let pale = draw.between(0.0, 1.0);
    let warm = draw.between(0.0, 1.0) * 0.02;
    [
        0.055 + pale * 0.13 + warm,
        0.058 + pale * 0.13,
        0.062 + pale * 0.125,
        1.0,
    ]
}

/// Bush green, kept darker than the ground cover so a bush reads as mass rather
/// than as very tall grass.
fn leaf(draw: &mut Draw) -> [f32; 4] {
    let light = draw.between(0.0, 1.0);
    [
        0.030 + light * 0.055,
        0.075 + light * 0.115,
        0.022 + light * 0.030,
        1.0,
    ]
}

const BARK: [f32; 4] = [0.052, 0.034, 0.022, 1.0];
const BRANCH: [f32; 4] = [0.042, 0.030, 0.021, 1.0];
/// The raw face of a break. Pale, and the reason a stump reads as a stump.
const HEARTWOOD: [f32; 4] = [0.30, 0.22, 0.135, 1.0];
/// Weathered dead wood: grey, not brown. Wood left standing goes silver.
const DEADWOOD: [f32; 4] = [0.155, 0.140, 0.118, 1.0];
const SUCCULENT: [f32; 4] = [0.045, 0.105, 0.040, 1.0];
/// Dead grass and dry stems.
const DRY: [f32; 4] = [0.170, 0.130, 0.062, 1.0];

#[cfg(test)]
mod tests {
    use super::*;

    fn any_ground(biome: Biome) -> Biome {
        biome
    }

    #[test]
    fn every_one_of_the_pool_grows_something() {
        for index in 0..VARIETIES {
            let prop = from_pool(index);
            assert!(
                !prop.mesh.is_empty(),
                "{} {index} grew nothing",
                prop.kind.name()
            );
            // Colour is the whole point of this pool: one material for the lot,
            // so a mesh that forgot to say what it was made of comes out white.
            assert_eq!(
                prop.mesh.colours.len(),
                prop.mesh.places.len(),
                "{} {index} has {} colours for {} vertices",
                prop.kind.name(),
                prop.mesh.colours.len(),
                prop.mesh.places.len()
            );
            assert!(
                (0.1..6.0).contains(&prop.reach),
                "{} {index} reaches {:.2} m",
                prop.kind.name(),
                prop.reach
            );
        }
    }

    #[test]
    fn every_face_faces_outward() {
        // The fault that made trees look transparent for a week, guarded on the
        // props too — they go through the same `Timber`, so they can catch it.
        for index in 0..VARIETIES {
            let prop = from_pool(index);
            let mut wrong = 0;
            let mut checked = 0;
            for face in prop.mesh.indices.chunks(3) {
                let [a, b, c] = [face[0], face[1], face[2]];
                let corner = |i: u32| Vec3::from_array(prop.mesh.places[i as usize]);
                let wound = (corner(b) - corner(a)).cross(corner(c) - corner(a));
                if wound.length_squared() < 1.0e-12 {
                    continue;
                }
                let says = [a, b, c]
                    .iter()
                    .map(|i| Vec3::from_array(prop.mesh.normals[*i as usize]))
                    .fold(Vec3::ZERO, |sum, n| sum + n);
                checked += 1;
                if wound.dot(says) < 0.0 {
                    wrong += 1;
                }
            }
            assert!(checked > 20, "{index}: only {checked} faces to check");
            assert_eq!(wrong, 0, "{} {index}: {wrong} inside-out", prop.kind.name());
        }
    }

    #[test]
    fn nothing_lies_about_in_the_water_or_in_a_town() {
        for biome in [Biome::Water, Biome::Settled] {
            assert_eq!(density(any_ground(biome)), 0.0);
            assert_eq!(pick(biome, 0.5, 0.5), None, "{biome:?} should stay clear");
        }
    }

    #[test]
    fn every_other_biome_has_something_that_belongs_in_it() {
        for biome in [
            Biome::Grass,
            Biome::Forest,
            Biome::Rock,
            Biome::Snow,
            Biome::Desert,
            Biome::Shore,
        ] {
            assert!(density(biome) > 0.0, "{biome:?} lies bare");
            // Every roll has to land on something, not just the middle one.
            for roll in [0.0, 0.33, 0.66, 0.999_f32] {
                let picked = pick(biome, roll, roll);
                assert!(picked.is_some(), "{biome:?} at {roll} picked nothing");
                assert!(picked.unwrap() < VARIETIES, "{biome:?} picked off the end");
            }
        }
    }

    #[test]
    fn a_wood_is_littered_and_a_meadow_is_not() {
        // The whole reason this is per-biome. A forest floor is a mess of its own
        // wreckage; grassland is grassland precisely because it is open, and a
        // meadow with a wood's worth of stumps in it is a wood.
        assert!(
            density(Biome::Forest) > density(Biome::Grass) * 2.5,
            "a wood should be far more littered than a field"
        );
        assert!(
            density(Biome::Rock) > density(Biome::Grass),
            "bare rock sheds stone; grass does not"
        );
    }

    #[test]
    fn the_pool_is_not_one_thing_repeated() {
        // Three variants of a kind that come out the same size are one variant
        // drawn three times, and a hillside of them shows it.
        for kind in Kind::ALL {
            let reaches: Vec<f32> = (0..VARIANTS)
                .map(|v| from_pool(kind.place() * VARIANTS + v).reach)
                .collect();
            let low = reaches.iter().copied().fold(f32::MAX, f32::min);
            let high = reaches.iter().copied().fold(0.0_f32, f32::max);
            assert!(
                high > low * 1.15,
                "{} comes out one size: {reaches:?}",
                kind.name()
            );
        }
    }

    #[test]
    fn the_same_seed_grows_the_same_thing() {
        // The contract with the bench: nothing about a prop is stored anywhere,
        // so both programs must grow the identical one from the identical index.
        let once = from_pool(7);
        let again = from_pool(7);
        assert_eq!(once.mesh.places, again.mesh.places);
        assert_eq!(once.mesh.colours, again.mesh.colours);
    }
}
