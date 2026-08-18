//! Ground cover: grass, flowers, and the dry scrub that stands in for both.
//!
//! The small stuff. A world can have good ground, good trees and good weather
//! and still read as a model of a world, because nothing grows on it — what
//! makes ground look like ground at walking height is that it is *littered*.
//!
//! # Merged, not scattered as things
//!
//! A tree is an entity: there are thousands and each is a separate mesh handle,
//! which an engine batches. Grass is HUNDREDS of thousands, and an entity apiece
//! would sink the frame rate before anything was drawn. So a patch of cover comes
//! out as ONE mesh — every tuft in a chunk welded together — and that is why the
//! colour lives in the vertices here rather than in a material the way a tree's
//! does. One mesh can only wear one material, and a meadow is not one green.
//!
//! # Where it grows is the biome's business
//!
//! Which is the whole reason [`crate::biome`] exists. Grass on grassland, less of
//! it under a wood, dry scrub in the desert, tufts on a shore, and nothing at all
//! on bare rock, in the snow, in open water or on ground somebody has levelled
//! and walks on.

use glam::{Vec2, Vec3};

use crate::biome::Biome;
use crate::Geometry;

/// Metres between slots on the cover lattice.
///
/// Every slot gets at most one tuft, so this sets the ceiling on how much there
/// can ever be: a 128 m chunk holds about six thousand slots at this spacing.
/// Tightening it multiplies the vertex count by the square, so it is the first
/// number to look at if the frame rate drops.
///
/// It was 2.6, then 1.7, then 1.15, and that is most of why the ground read as
/// sprigs
/// dotted about rather than as grass. A tuft is a hand's width across; one every
/// two and a half metres is not a sward however many of them there are, because
/// the eye reads the GAPS.
///
/// What pays for closing them is that cover clumps — see [`patch`] — so almost
/// all of this is spent inside meadows and hardly any of it on the bare ground
/// between, where it would only be stubble again.
pub const SPACING: f32 = 1.0;

/// What is growing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sprig {
    /// A tuft of blades. Most of everything.
    Grass,
    /// A tuft with a head of colour on it.
    Flower,
    /// Stiff, dry, splayed. What passes for grass where there is no water.
    ///
    /// Unreachable as things stand: dry country carries no cover at all now, and
    /// what it has instead is cactus and dead brush, which are objects rather
    /// than sprigs. Kept because it is one number in [`density`] from coming
    /// back, and because deleting a shape to save nothing is not a saving.
    Scrub,
}

/// How strongly the cover clumps at a point: 0 on bare ground between patches,
/// 1 in the middle of a meadow.
///
/// # Grass grows in patches, and that is the whole point of this
///
/// Spread a biome's worth of grass evenly and every field in the world carries
/// the same thin stubble — which is what it looked like. Real ground is not like
/// that: grass goes where the water and the light and the soil are, so it comes
/// in meadows with thinner ground between them, and it is the PATCHES that make
/// somewhere look like a place rather than a texture.
///
/// So the same amount of grass is gathered rather than spread. A patch core is
/// solid and taller, the ground between is nearly bare, and the average over a
/// hillside is about what it was — which is why this costs nothing.
///
/// Two octaves of smooth value noise off the shared hash, at forty metres and at
/// fifteen. The big one lays out the meadows and the small one keeps their edges
/// from being circles.
///
/// Not every biome clumps, and desert is the reason this takes one at all. Dry
/// scrub is sporadic BY NATURE — that is what makes it read as dry — so gathering
/// it into lush patches would be inventing oases. It, and bare rock and snow, are
/// told to stay as they are.
pub fn patch(biome: Biome, at: Vec2) -> f32 {
    if !matches!(
        biome,
        Biome::Grass | Biome::Forest | Biome::Shore | Biome::Settled
    ) {
        return SPREAD_EVENLY;
    }

    let rough = field(at, MEADOW_WIDE, SALT_PATCH_WIDE) * 0.65
        + field(at, MEADOW_FINE, SALT_PATCH_FINE) * 0.35;
    // Squared off into actual patches. Left as it comes, the field is a gentle
    // wobble and the grass merely gets slightly thicker in places; pushed
    // through a smoothstep, it has middles and edges and gaps.
    crate::smoothstep(MEADOW_EDGE.0, MEADOW_EDGE.1, rough)
}

/// Smooth value noise over a lattice of the given size, from the shared hash.
fn field(at: Vec2, cell: f32, salt: u32) -> f32 {
    let on = at / cell;
    let low = on.floor();
    let across = on - low;
    // Eased both ways, or the patches come out as diamonds with straight sides.
    let ease = Vec2::new(
        across.x * across.x * (3.0 - 2.0 * across.x),
        across.y * across.y * (3.0 - 2.0 * across.y),
    );

    let corner = |step_x: i32, step_z: i32| {
        chance(low.x as i32 + step_x, low.y as i32 + step_z, salt)
    };
    let near = corner(0, 0) * (1.0 - ease.x) + corner(1, 0) * ease.x;
    let far = corner(0, 1) * (1.0 - ease.x) + corner(1, 1) * ease.x;
    near * (1.0 - ease.y) + far * ease.y
}

/// How tall the tallest tuft this crate grows can stand, in metres.
///
/// Named rather than left to be worked out from `HEIGHT` and `STATURE` by anyone
/// who needs it. A camera has to clear the grass it skims, and a game that
/// re-derived this from two private constants would drift out of step with the
/// grass the moment either moved.
pub fn tallest() -> f32 {
    HEIGHT * STATURE.1
}

/// How big a tuft stands, given how deep in a patch it is.
///
/// Grass in the middle of a meadow is taller than grass at the edge of one,
/// because that is where the growing is good — and a patch that is only DENSER
/// than its surroundings reads as more of the same rather than as a meadow.
pub fn stature(patch: f32) -> f32 {
    STATURE.0 + (STATURE.1 - STATURE.0) * patch
}

/// How much cover a place carries, 0 to 1.
///
/// `sureness` is [`Biome::confidence`] — how strongly the ground reads as its own
/// kind. Cover fades with it rather than stopping at a boundary, so a meadow
/// thins into a wood instead of ending along a line.
///
/// `patch` is [`patch`]: the same amount of grass, gathered instead of spread.
pub fn density(biome: Biome, sureness: f32, patch: f32) -> f32 {
    thinly(biome, sureness) * (THICKNESS.0 + (THICKNESS.1 - THICKNESS.0) * patch)
}

/// What a biome would carry if it were spread evenly — the average a patch field
/// is swung either side of.
fn thinly(biome: Biome, sureness: f32) -> f32 {
    let most = match biome {
        // Open country is what grass is for, and now it is the only place it is.
        Biome::Grass => 0.85,
        // A wood floor keeps some. Less than open country because less light
        // reaches it, and it is the one place on this list that is a judgement
        // rather than an instruction — a forest floor scrubbed bare reads as a
        // park. One number from going either way.
        Biome::Forest => 0.34,
        // And nowhere else grows a blade of it.
        //
        // Sand, rock, snow, desert and the ground a town stands on had a little
        // each, on the reasoning that nowhere real is completely bare. That is
        // true and it was the wrong call: what those places actually needed was
        // things that belong in THEM, and they have those now — driftwood on a
        // shore, scree on the mountain, cactus and dead brush in the desert. A
        // thin scatter of meadow grass over the top of all of it only made five
        // different places look like the same place with different ground paint.
        //
        // It is also most of a world's worth of grass that no longer has to be
        // built, meshed and drawn.
        Biome::Shore | Biome::Desert | Biome::Rock | Biome::Snow => 0.0,
        // A town is somebody's. Grass through the market square is the same fault
        // the rivers had and the boulders nearly had.
        Biome::Settled => 0.0,
        // And nothing at all grows in open water.
        Biome::Water => 0.0,
    };
    // Never the full amount right at a boundary: cover is the first thing to
    // thin out as ground stops being what it was.
    most * (0.55 + 0.45 * sureness)
}

/// What grows here, given a roll for variety.
///
/// Flowers are a fraction of the grass rather than a thing of their own: a meadow
/// is grass with flowers IN it, and scattering them on their own lattice put them
/// in tidy rows of their own.
pub fn kind(biome: Biome, roll: f32) -> Sprig {
    match biome {
        Biome::Desert => Sprig::Scrub,
        // Rock and snow carry the odd hardy tuft and never a flower. Nor does
        // trodden ground: a yard people cross is grass and weeds, not a meadow.
        Biome::Rock | Biome::Snow | Biome::Settled => Sprig::Grass,
        Biome::Shore => {
            if roll < 0.08 {
                Sprig::Flower
            } else {
                Sprig::Grass
            }
        }
        // A meadow is about a twelfth flowers; a wood floor rather fewer.
        Biome::Forest => {
            if roll < 0.05 {
                Sprig::Flower
            } else {
                Sprig::Grass
            }
        }
        _ => {
            if roll < 0.09 {
                Sprig::Flower
            } else {
                Sprig::Grass
            }
        }
    }
}

/// A repeatable 0..1 from a slot and a purpose.
///
/// The forest's hash, with its own salts. Shared deliberately: two programs
/// drawing the same meadow must scatter it identically, and one hash with
/// separate salts is how the woods already manage it.
pub fn chance(x: i32, z: i32, salt: u32) -> f32 {
    crate::forest::chance(x, z, salt)
}

/// How far a patch swings the thickness either side of the biome's average.
///
/// Bare between patches and well over the top inside them. Past one means every
/// slot in a core carries a tuft, so a core is SOLID rather than merely thick —
/// and the low end is deliberately almost nothing, because every tuft not spent
/// on stubble between the meadows is one that can be spent inside them.
const THICKNESS: (f32, f32) = (0.06, 1.9);

/// How much of a full turn a tuft's blades are spread through, least and most.
///
/// Never the whole circle, and that is the point — a tuft spread right round is a
/// rosette, and every rosette is every other rosette turned a bit. Half a turn to
/// four fifths gives a clump with a front and a back, so which way it faces
/// becomes something that can differ between one and the next.
const SWEEP: (f32, f32) = (0.5, 0.82);

/// How far a whole clump leans off upright, as a share of its own splay.
const TILT: f32 = 0.5;

/// How many more blades a tuft grows in the middle of a patch than at its edge.
///
/// The other half of how thick a thicket is, and the half that keeps working
/// after the first one stops. Once a core is dense enough that every slot on the
/// lattice carries a tuft — which it is — the only ways left to fill it are to
/// put the slots closer together or to make each tuft fuller, and a blade added
/// to a tuft that already exists is cheaper than a whole new tuft.
const LUSH_BLADES: usize = 4;

/// How much deeper in colour a tuft is in the middle of a patch.
const LUSH_SHADE: f32 = 0.3;

/// How much taller a tuft stands in the middle of a patch than at its edge.
///
/// Three times over, which is a lot and is the point. Grass that is merely
/// thicker in places reads as a slightly better lawn; grass that comes up past
/// your knee is a different KIND of ground, and you can see from across a field
/// where it starts and stops. That is what makes it somewhere a wild monster
/// lives rather than decoration.
const STATURE: (f32, f32) = (0.7, 2.3);

/// What a biome that does not clump gets told.
const SPREAD_EVENLY: f32 = 0.5;

/// How wide the meadows are, in metres, and how wide the detail on their edges.
const MEADOW_WIDE: f32 = 40.0;
const MEADOW_FINE: f32 = 15.0;

/// The band the patch field is squared off across.
///
/// Narrow, so there are middles and gaps rather than a wobble; not so narrow that
/// a meadow has a hard rim. Centred a little above a half, which is what leaves
/// rather more open ground than thicket — a field with tall grass in it, not a
/// thicket with clearings.
///
/// Tighter than it was, because a player has to be able to SEE where the tall
/// grass begins to decide whether to walk into it.
const MEADOW_EDGE: (f32, f32) = (0.44, 0.62);

/// Salts the cover uses. Numbered clear of the forest's, which owns 1 to 7.
pub const SALT_JITTER_X: u32 = 11;
pub const SALT_JITTER_Z: u32 = 12;
pub const SALT_PRESENT: u32 = 13;
pub const SALT_KIND: u32 = 14;
pub const SALT_TURN: u32 = 15;
pub const SALT_SCALE: u32 = 16;
pub const SALT_SHADE: u32 = 17;
pub const SALT_PETAL: u32 = 18;
/// And the two the meadows are laid out with.
pub const SALT_PATCH_WIDE: u32 = 19;
pub const SALT_PATCH_FINE: u32 = 20;

/// The greens grass is drawn between, darkest first, as linear RGB.
///
/// The palette lives here rather than in a game because the colour is baked into
/// the vertices, and the vertices are made here. A tree can be tinted by its
/// material afterwards; a welded meadow cannot.
const GRASS_DARK: [f32; 3] = [0.055, 0.14, 0.035];
const GRASS_LIGHT: [f32; 3] = [0.16, 0.30, 0.075];
/// And the dry pair, for scrub.
const SCRUB_DARK: [f32; 3] = [0.16, 0.14, 0.06];
const SCRUB_LIGHT: [f32; 3] = [0.30, 0.26, 0.11];

/// What a flower head can be. Sparse enough that a handful of hues reads as
/// variety rather than as a pattern.
const PETALS: [[f32; 3]; 5] = [
    [0.62, 0.60, 0.55],  // white
    [0.60, 0.50, 0.05],  // yellow
    [0.42, 0.10, 0.28],  // pink
    [0.14, 0.14, 0.42],  // blue
    [0.46, 0.16, 0.06],  // orange
];

/// How tall a tuft stands, in metres, before its own scale is applied.
///
/// A patch core takes this to well over a metre and a half — see [`STATURE`].
///
/// Which is not how tall the grass STANDS. A blade arches over and droops, so it
/// reaches about three fifths of its own length into the air; the length had to
/// grow when the blades started bending, or the same grass that used to come up
/// past the knee would have come up to the shin. What matters is the standing
/// height, and that is deliberately waist-high on a person: tall grass is
/// somewhere a monster can be without being seen, and grass you can see over is
/// scenery instead of cover.
const HEIGHT: f32 = 0.72;

/// Adds one tuft to a mesh being built.
///
/// `turn` spins it about its own base, `scale` sizes it, and `shade` and `petal`
/// are rolls for its colour. Everything is appended in place, because a chunk's
/// worth of these is one mesh and building it is one loop.
// Eight, and clippy is right to count them. They are eight because a tuft is
// decided by eight independent rolls, and bundling them into a struct would move
// the same eight one line up and add a name nobody would otherwise need.
#[allow(clippy::too_many_arguments)]
pub fn add(
    into: &mut Geometry,
    kind: Sprig,
    at: Vec3,
    turn: f32,
    scale: f32,
    shade: f32,
    petal: f32,
    lush: f32,
) {
    let (dark, light, blades) = match kind {
        Sprig::Grass => (GRASS_DARK, GRASS_LIGHT, 3),
        Sprig::Flower => (GRASS_DARK, GRASS_LIGHT, 2),
        // Splayed and stiff, and more of them: a scrub clump is what a dry place
        // has instead of a sward.
        Sprig::Scrub => (SCRUB_DARK, SCRUB_LIGHT, 5),
    };

    // Fuller the deeper into a patch it stands. Tall grass has to be THICK as
    // well as tall — a tuft of four blades stretched to a metre is a spider, not
    // a thicket — and a blade costs one triangle, so this is the cheapest kind of
    // density there is.
    //
    // Scrub is left alone. Dry country's clumps are sparse by nature and `patch`
    // never gathers them anyway.
    let blades = if kind == Sprig::Scrub {
        blades
    } else {
        blades + (LUSH_BLADES as f32 * lush).round() as usize
    };
    // And not the same count twice running. Two tufts of identical size with an
    // identical number of blades are the same object however their blades are
    // jittered, and a field of one object is a field of one object.
    let blades = (blades + (fract(shade * 29.3) * 3.0) as usize).max(3);

    // How much of a full turn this tuft's blades are spread through.
    //
    // # This is why they all looked the same
    //
    // Every tuft fanned through the whole circle. That is a rosette by
    // construction — turn one and you get the same tuft back, so no amount of
    // jittering the angles inside it makes two of them different objects. Grass
    // does not grow in rosettes; it grows in clumps that face somewhere.
    //
    // Spread through half a turn and the tuft has a front and a back, and `turn`
    // — which was doing nothing visible on a rosette — now decides which way it
    // faces.
    let sweep = SWEEP.0 + (SWEEP.1 - SWEEP.0) * fract(shade * 13.7);

    // And the whole clump leans, rather than splaying evenly about its own
    // middle. A tuft that leans is a tuft with a mood; a field of them has wind
    // in it even when nothing is moving.
    let tilting = fract(shade * 5.21) * std::f32::consts::TAU;
    let tilt = Vec3::new(tilting.cos(), 0.0, tilting.sin()) * TILT * fract(shade * 11.9);

    // And deeper in colour. A thicket is shaded by its own depth, which is what
    // makes a patch of it read as a mass rather than as more of the same grass.
    let green = shade_of(mix(dark, light, shade), 1.0 - LUSH_SHADE * lush);
    let tall = HEIGHT * scale;
    // Wider too, in step with how tall it has grown, or a metre-high blade comes
    // out as a wire.
    let wide = 0.020 * scale * (1.0 + 0.5 * lush);
    // How far across the ground the blades rise from. Scrub sprawls; grass keeps
    // to its own clump.
    let clump = wide * if kind == Sprig::Scrub { 3.2 } else { 2.1 };
    // How far past upright a blade's tip has turned, as a share of a right angle.
    //
    // Over one and the tip is heading DOWNWARD — the blade has flopped under its
    // own weight, which is what grass does and what a spike cannot do however it
    // is bent. Scrub stops just short of it: splayed out nearly flat and holding
    // itself there is what dry, stiff growth looks like.
    let leaning = if kind == Sprig::Scrub { 0.92 } else { 1.15 };

    for blade in 0..blades {
        // Two rolls of its own, so no two blades in a tuft agree about anything.
        let roll = fract(shade * 7.13 + blade as f32 * 0.618_034);
        let sway = fract(shade * 3.77 + blade as f32 * 0.381_966 + 0.5);

        // # Why a tuft used to read as a crown
        //
        // Blades spread at even steps right round a circle, every one the same
        // length, every one leaning out by the same amount, every one rising from
        // the same point. That is not a description of grass — it is the
        // construction of a coronet, and it came out looking like one.
        //
        // So the even step is jittered hard, by most of the gap between blades.
        let angle = turn
            + (blade as f32 + (roll - 0.5) * 1.5) / blades as f32
                * std::f32::consts::TAU
                * sweep;
        let out = Vec3::new(angle.cos(), 0.0, angle.sin());

        // And they rise from a patch of ground rather than from a point. The
        // pinch where every blade converged is what gave a tuft its stem, and a
        // stem under a fan of spikes is exactly a crown.
        let foot = at + out * clump * (0.2 + 0.8 * sway);

        // Lengths that actually differ. A quarter's variation reads as one
        // length badly cut; better than half of it reads as grass.
        let length = tall * (0.45 + 0.55 * roll);
        // A narrow spread, because this is an angle now and not a distance. Half
        // again either way and some blades curl back on themselves.
        let lean = leaning * (0.75 + 0.5 * sway);

        // Darker at the root and lighter at the tip: the cheapest thing that
        // stops a field of blades reading as flat paint.
        blade_into(
            into,
            foot,
            // Its own splay plus the whole clump's lean. Normalising afterwards
            // would throw the lean away again, so the two are simply added and
            // the blade goes where the sum points.
            (out + tilt).normalize_or(out),
            length,
            lean,
            wide * (0.7 + 0.5 * sway),
            shade_of(green, 0.72),
            green,
        );
    }

    if kind == Sprig::Flower {
        // A head of three petals on top. Not a real flower and not trying to be:
        // at walking height what registers is a spot of colour above the grass.
        let colour = PETALS[(petal * PETALS.len() as f32) as usize % PETALS.len()];
        let head = at + Vec3::Y * tall * 0.92;
        let span = 0.055 * scale;
        for petal_index in 0..3 {
            let angle = turn + petal_index as f32 / 3.0 * std::f32::consts::TAU;
            let out = Vec3::new(angle.cos(), 0.0, angle.sin());
            blade_into(into, head, out, span * 1.6, 0.9, span, colour, colour);
        }
    }
}

/// One blade: a narrow ribbon that arches over and droops at the tip.
///
/// # It was a spike, and then it was a bent spike
///
/// The first version was one triangle — wide at the foot, needle-pointed at the
/// top, dead straight. A ring of those is an agave, and from above it is a black
/// starfish. Bending it in the middle helped from the side and not at all from
/// overhead, because the thing was still a wedge with a point on it.
///
/// A blade of grass is not a wedge. It is a RIBBON: near enough the same narrow
/// width along almost its whole length, tapering only in the last stretch, and it
/// does not point anywhere — it leaves the ground steeply, bends over under its
/// own weight, and by the tip it is heading back down.
///
/// So the shape is swept along an arc instead of drawn between two ends. Each
/// step turns a little further from upright than the last, which is what makes it
/// curve continuously rather than kink; four stations is enough for the eye to
/// read an arch, and past a right angle the tip is falling, which is the
/// silhouette that says grass and nothing else does.
///
/// Seven vertices. The width is the other half of it, and it is the half that
/// costs: a blade's width is what decides how many PIXELS it covers, and a meadow
/// overdraws itself many times over. Going from a wedge four centimetres at the
/// foot to a ribbon of one and a third put the vertex count up a fifth and the
/// fragment count down by thirty per cent, at the same frame cost — which is how
/// it came out that grass had never been vertex-bound at all.
///
/// So this is the number to be careful with, and the one to reach for if the
/// frame ever needs headroom back.
#[allow(clippy::too_many_arguments)]
fn blade_into(
    into: &mut Geometry,
    foot: Vec3,
    out: Vec3,
    length: f32,
    lean: f32,
    width: f32,
    root_colour: [f32; 3],
    tip_colour: [f32; 3],
) {
    const STEPS: usize = 3;
    // `lean` is how far past upright the tip ends up, in right angles.
    let over = lean * std::f32::consts::FRAC_PI_2;

    let across = Vec3::new(-out.z, 0.0, out.x);
    let step = length / STEPS as f32;

    let base = into.places.len() as u32;
    // Upright rather than surface-true. A blade's own normal points sideways, so
    // lighting it honestly makes a meadow flicker dark as the camera turns;
    // facing them up lights the field like the ground it belongs to.
    let up = [0.0, 1.0, 0.0];

    let mut at = foot;
    for station in 0..=STEPS {
        let along = station as f32 / STEPS as f32;
        let colour = mix(root_colour, tip_colour, along);

        if station == STEPS {
            // The tip is one point, which is where the taper ends.
            into.places.push(at.to_array());
            into.normals.push(up);
            into.uvs.push([1.0, 0.5]);
            into.colours.push([colour[0], colour[1], colour[2], 1.0]);
            break;
        }

        // Barely tapering until the last stretch, which is what makes it a
        // ribbon rather than a wedge.
        let half = across * width * 0.5 * (1.0 - along * along * along * 0.85);
        for side in [-1.0, 1.0_f32] {
            into.places.push((at + half * side).to_array());
            into.normals.push(up);
            into.uvs.push([along, 0.5]);
            into.colours.push([colour[0], colour[1], colour[2], 1.0]);
        }

        // Turned a little further from upright for every step taken, so the
        // blade curves rather than folding at one joint.
        let turned = over * ((station + 1) as f32 / STEPS as f32).powf(1.6);
        at += (Vec3::Y * turned.cos() + out * turned.sin()) * step;
    }

    // Both faces of every triangle. A blade is a strip with no thickness, so half
    // a meadow would otherwise be missing from any given angle.
    let mut face = |a: u32, b: u32, c: u32| {
        into.indices
            .extend_from_slice(&[base + a, base + b, base + c]);
        into.indices
            .extend_from_slice(&[base + a, base + c, base + b]);
    };
    for rung in 0..STEPS as u32 - 1 {
        let (low, high) = (rung * 2, rung * 2 + 2);
        face(low, low + 1, high + 1);
        face(low, high + 1, high);
    }
    let last = (STEPS as u32 - 1) * 2;
    face(last, last + 1, STEPS as u32 * 2);
}

fn mix(low: [f32; 3], high: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        low[0] + (high[0] - low[0]) * t,
        low[1] + (high[1] - low[1]) * t,
        low[2] + (high[2] - low[2]) * t,
    ]
}

fn shade_of(colour: [f32; 3], by: f32) -> [f32; 3] {
    [colour[0] * by, colour[1] * by, colour[2] * by]
}

fn fract(value: f32) -> f32 {
    value - value.floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grass_gathers_into_patches_and_desert_does_not() {
        // The fault this fixes: cover spread evenly is the same thin stubble on
        // every field in the world. Gathering the SAME amount into meadows is
        // what makes somewhere look like a place.
        let mut most = 0.0_f32;
        let mut least = 1.0_f32;
        let mut total = 0.0;
        let mut looked = 0;

        // Across a few hundred metres, which is several meadows wide.
        for step_z in 0..80 {
            for step_x in 0..80 {
                let at = Vec2::new(step_x as f32 * 5.0, step_z as f32 * 5.0);
                let patch = patch(Biome::Grass, at);
                most = most.max(patch);
                least = least.min(patch);
                total += patch;
                looked += 1;
            }
        }

        assert!(most > 0.95, "no meadow ever reaches its middle: {most:.2}");
        assert!(least < 0.05, "there is no bare ground between them: {least:.2}");

        // And it has to be patches on a field, not a field with bald spots.
        let mean = total / looked as f32;
        assert!(
            (0.2..0.55).contains(&mean),
            "meadows cover {:.0}% of the ground",
            mean * 100.0
        );

        // Costing about what evenly-spread cover did, which is why it is free.
        let spread = density(Biome::Grass, 1.0, SPREAD_EVENLY);
        let gathered = density(Biome::Grass, 1.0, mean);
        assert!(
            (gathered / spread - 1.0).abs() < 0.35,
            "gathering the grass changed how much there is by {:.0}%",
            (gathered / spread - 1.0) * 100.0
        );

        // Dry country is sporadic BY NATURE — that is what makes it read as dry —
        // so it must not be gathered into oases.
        for bare in [Biome::Desert, Biome::Rock, Biome::Snow] {
            let corners: Vec<f32> = (0..40)
                .map(|step| patch(bare, Vec2::new(step as f32 * 37.0, step as f32 * 23.0)))
                .collect();
            assert!(
                corners.windows(2).all(|w| w[0] == w[1]),
                "{bare:?} is being gathered into patches"
            );
        }
    }

    #[test]
    fn a_meadow_is_solid_and_the_ground_between_is_not() {
        // A patch has to be thicker AND taller than what surrounds it. One
        // without the other reads as more of the same rather than as a meadow.
        let core = density(Biome::Grass, 1.0, 1.0);
        let gap = density(Biome::Grass, 1.0, 0.0);
        assert!(core >= 1.0, "a meadow's middle is not solid: {core:.2}");
        assert!(gap < 0.2, "the ground between meadows is not bare: {gap:.2}");
        assert!(stature(1.0) > stature(0.0) * 1.4, "a meadow is no taller");
    }

    #[test]
    fn nothing_grows_in_open_water() {
        assert_eq!(density(Biome::Water, 1.0, SPREAD_EVENLY), 0.0);
    }

    #[test]
    fn grass_grows_only_where_grass_grows() {
        // Sand, rock, snow, desert and the ground a town stands on each used to
        // carry a thin scatter, on the reasoning that nowhere real is completely
        // bare. True, and the wrong call: it made five different places look like
        // one place with different ground paint. What they needed was things that
        // belong in THEM, and they have those — driftwood, scree, cactus, brush.
        for bare in [
            Biome::Shore,
            Biome::Desert,
            Biome::Rock,
            Biome::Snow,
            Biome::Settled,
            Biome::Water,
        ] {
            for patch in [0.0, 0.5, 1.0] {
                assert_eq!(
                    density(bare, 1.0, patch),
                    0.0,
                    "{} should carry no cover at all",
                    bare.name()
                );
            }
        }

        // And the two that do, in the right order.
        let meadow = density(Biome::Grass, 1.0, SPREAD_EVENLY);
        let wood = density(Biome::Forest, 1.0, SPREAD_EVENLY);
        assert!(wood > 0.0, "a wood floor should not be swept");
        assert!(meadow > wood, "open country should carry more than a wood");
    }

    #[test]
    fn open_country_carries_the_most_and_a_wood_carries_less() {
        let meadow = density(Biome::Grass, 1.0, SPREAD_EVENLY);
        let under_trees = density(Biome::Forest, 1.0, SPREAD_EVENLY);
        let dry = density(Biome::Desert, 1.0, SPREAD_EVENLY);
        assert!(meadow > under_trees, "{meadow} vs {under_trees}");
        assert!(under_trees > dry, "{under_trees} vs {dry}");
    }

    #[test]
    fn cover_thins_toward_a_boundary_rather_than_stopping_at_it() {
        // A meadow should fade into a wood, not end along a line somebody can
        // see from the air.
        let sure = density(Biome::Grass, 1.0, SPREAD_EVENLY);
        let edge = density(Biome::Grass, 0.0, SPREAD_EVENLY);
        assert!(edge < sure, "{edge} should be under {sure}");
        assert!(edge > 0.0, "and not nothing at all");
    }

    #[test]
    fn a_desert_grows_scrub_and_never_a_flower() {
        for step in 0..64 {
            let roll = step as f32 / 63.0;
            assert_eq!(kind(Biome::Desert, roll), Sprig::Scrub);
            assert_ne!(kind(Biome::Rock, roll), Sprig::Flower);
            assert_ne!(kind(Biome::Snow, roll), Sprig::Flower);
        }
    }

    #[test]
    fn a_meadow_is_mostly_grass_with_flowers_in_it() {
        let flowers = (0..1000)
            .filter(|step| kind(Biome::Grass, *step as f32 / 999.0) == Sprig::Flower)
            .count();
        // Some, and nowhere near most. A field of flowers is a different thing.
        assert!((30..200).contains(&flowers), "{flowers} in a thousand");
    }

    #[test]
    fn tall_grass_is_tall_enough_to_lose_something_in() {
        // The point of a thicket is that a wild monster can be in it without
        // being seen. Grass you can see over is scenery.
        let reach = |lush: f32| {
            let mut mesh = Geometry::default();
            add(
                &mut mesh,
                Sprig::Grass,
                Vec3::ZERO,
                0.0,
                stature(lush),
                0.5,
                0.0,
                lush,
            );
            (
                mesh.places.iter().map(|p| p[1]).fold(0.0_f32, f32::max),
                mesh.indices.len(),
            )
        };

        let (thin, thin_faces) = reach(0.0);
        let (thick, thick_faces) = reach(1.0);

        assert!(
            thick > 0.9,
            "the middle of a patch is only {thick:.2} m tall — you can see over it"
        );
        assert!(
            thin < 0.5,
            "the edge of a patch is {thin:.2} m tall, which is not an edge"
        );
        // And thick as well as tall. A few blades stretched to a metre is a
        // spider, not a thicket.
        //
        // Half again rather than twice: how many blades a tuft grows is now
        // partly its own roll, so the ratio between a lush one and a thin one is
        // not a fixed multiple, and pinning it to one would be a test of the
        // arithmetic rather than of the claim.
        assert!(
            thick_faces * 2 > thin_faces * 3,
            "a thicket tuft has {thick_faces} triangles against a thin one's {thin_faces}"
        );
    }

    #[test]
    fn a_tuft_stands_on_the_ground_and_reaches_up_from_it() {
        let mut mesh = Geometry::default();
        add(&mut mesh, Sprig::Grass, Vec3::ZERO, 0.0, 1.0, 0.5, 0.0, 1.0);

        assert!(!mesh.places.is_empty(), "a tuft should have geometry");
        assert_eq!(
            mesh.colours.len(),
            mesh.places.len(),
            "every vertex needs its colour, or the mesh is refused"
        );

        let lowest = mesh.places.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        let highest = mesh.places.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        assert!(lowest > -0.01, "a tuft should not be planted underground: {lowest}");
        assert!(
            (0.2..0.6).contains(&highest),
            "and should stand about ankle high: {highest}"
        );
    }

    #[test]
    fn a_flower_carries_colour_that_is_not_a_green() {
        let mut mesh = Geometry::default();
        add(&mut mesh, Sprig::Flower, Vec3::ZERO, 0.0, 1.0, 0.5, 0.35, 1.0);
        // Somewhere in it there has to be a vertex whose red beats its green,
        // which no blade of grass ever has.
        assert!(
            mesh.colours.iter().any(|c| c[0] > c[1]),
            "a flower should have a head on it"
        );
    }

    #[test]
    fn no_two_tufts_are_the_same_tuft() {
        // "Those are the same thing." They were: every tuft fanned through a
        // whole circle, which is a rosette, and every rosette is every other one
        // turned a bit. Jittering the angles inside it cannot help — the object
        // is symmetric, so the variation averages back out.
        //
        // What is checked here is the SILHOUETTE, because that is what the eye
        // compares: how far the blades reach, which way the clump faces, and how
        // many of them there are.
        // Turned as well as shaded, because that is how a chunk plants them —
        // holding `turn` at zero would line every clump up facing the same way
        // and then blame the tuft for it.
        let sketch = |shade: f32, turn: f32| {
            let mut mesh = Geometry::default();
            add(&mut mesh, Sprig::Grass, Vec3::ZERO, turn, 1.0, shade, 0.0, 1.0);
            let middle = mesh
                .places
                .iter()
                .filter(|p| p[1] > 0.2)
                .fold(Vec2::ZERO, |sum, p| sum + Vec2::new(p[0], p[2]))
                / mesh.places.iter().filter(|p| p[1] > 0.2).count().max(1) as f32;
            (mesh.places.len(), middle)
        };

        let tufts: Vec<(usize, Vec2)> = (0..12)
            .map(|n| {
                sketch(
                    n as f32 / 12.0,
                    chance(n, n * 7, SALT_TURN) * std::f32::consts::TAU,
                )
            })
            .collect();

        // Different numbers of blades between them.
        let fewest = tufts.iter().map(|t| t.0).min().unwrap();
        let most = tufts.iter().map(|t| t.0).max().unwrap();
        assert!(
            most > fewest,
            "every tuft has exactly {most} vertices — they are one object"
        );

        // And they face somewhere. A rosette's blades average back to its own
        // middle; a clump's do not, and two clumps lean different ways.
        let leans: Vec<f32> = tufts.iter().map(|t| t.1.length()).collect();
        let leaniest = leans.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            leaniest > 0.02,
            "every tuft is symmetric about its own middle ({leaniest:.3} m) — that is a rosette"
        );

        // Facing different ways, not all the same way.
        let facings: Vec<Vec2> = tufts
            .iter()
            .filter(|t| t.1.length() > 0.01)
            .map(|t| t.1.normalize())
            .collect();
        assert!(facings.len() > 4, "too few tufts lean to judge");
        let together = facings
            .iter()
            .fold(Vec2::ZERO, |sum, facing| sum + *facing)
            .length()
            / facings.len() as f32;
        assert!(
            together < 0.6,
            "every tuft leans the same way ({together:.2}) — that is one object again"
        );
    }

    #[test]
    fn a_blade_is_a_ribbon_that_arches_over() {
        // Twice now the shape has been the complaint, and both times it was the
        // same fault: a blade drawn as a WEDGE. Wide at the foot, needle-pointed
        // at the top, and pointing wherever it was aimed — which is an agave
        // leaf, and a ring of them seen from above is a black starfish.
        let mut mesh = Geometry::default();
        add(&mut mesh, Sprig::Grass, Vec3::ZERO, 0.0, 1.0, 0.5, 0.0, 1.0);

        // Take one blade: the vertices in order along it, by how far up they say
        // they are.
        let along = |want: f32| -> Vec<Vec3> {
            mesh.uvs
                .iter()
                .zip(&mesh.places)
                .filter(|(uv, _)| (uv[0] - want).abs() < 0.01)
                .map(|(_, place)| Vec3::from_array(*place))
                .take(2)
                .collect()
        };

        // A ribbon: still most of its width two thirds of the way up. A wedge has
        // lost most of it by then.
        let foot = along(0.0);
        let high = along(2.0 / 3.0);
        assert_eq!(foot.len(), 2, "a blade should have two edges at its foot");
        assert_eq!(high.len(), 2, "and two most of the way up");
        let at_foot = (foot[0] - foot[1]).length();
        let up_top = (high[0] - high[1]).length();
        assert!(
            up_top > at_foot * 0.6,
            "a blade is down to {:.0}% of its width two thirds up — that is a wedge",
            up_top / at_foot * 100.0
        );

        // And it arches: by the tip it is heading DOWN, not up. Which is the one
        // thing a spike can never do however it is bent.
        let tip = mesh
            .uvs
            .iter()
            .zip(&mesh.places)
            .find(|(uv, _)| uv[0] > 0.99)
            .map(|(_, place)| Vec3::from_array(*place))
            .expect("a blade should have a tip");
        let last = along(2.0 / 3.0)[0];
        assert!(
            tip.y < last.y,
            "the tip is still climbing ({:.2} m against {:.2}) — it has not flopped over",
            tip.y,
            last.y
        );

        // Narrow. A centimetre reads as a blade of grass; four reads as a leaf.
        assert!(
            at_foot < 0.05,
            "a blade is {:.0} cm across at the foot",
            at_foot * 100.0
        );
    }

    #[test]
    fn a_tuft_is_not_a_crown() {
        // What it looked like, and why. Blades at even steps right round a
        // circle, all the same length, all leaning the same amount, all rising
        // from one point — that is the construction of a coronet, and it came out
        // looking like one.
        let mut mesh = Geometry::default();
        add(&mut mesh, Sprig::Grass, Vec3::ZERO, 0.0, 1.0, 0.5, 0.0, 1.0);

        let feet: Vec<Vec3> = mesh
            .places
            .iter()
            .map(|p| Vec3::from_array(*p))
            .filter(|p| p.y < 0.01)
            .collect();
        assert!(feet.len() >= 6, "a tuft should stand on several blades");

        // The blades rise from a patch of ground, not from a stem. A crown's
        // feet all sit within a hair of each other.
        let spread = feet
            .iter()
            .map(|foot| Vec3::new(foot.x, 0.0, foot.z).length())
            .fold(0.0_f32, f32::max);
        assert!(
            spread > 0.02,
            "every blade rises from the same point ({spread:.3} m across) — that is a stem"
        );

        // And they are not all the same length.
        let tips: Vec<f32> = mesh
            .uvs
            .iter()
            .zip(&mesh.places)
            .filter(|(uv, _)| uv[0] > 0.99)
            .map(|(_, place)| place[1])
            .collect();
        let tallest = tips.iter().copied().fold(0.0_f32, f32::max);
        let shortest = tips.iter().copied().fold(f32::MAX, f32::min);
        assert!(
            shortest < tallest * 0.75,
            "every blade is the same height: {shortest:.2} against {tallest:.2}"
        );
    }

    #[test]
    fn every_blade_faces_two_ways() {
        // A blade is a strip with no thickness, so half a meadow would be
        // invisible from any given side if it were wound once.
        //
        // This used to check that there were twice as many indices as vertices,
        // which was true only while a blade was one triangle of three corners.
        // It is a bent strip of three triangles now, and counting was never the
        // invariant anyway: what matters is that every triangle has its own
        // reverse somewhere in the mesh.
        let mut mesh = Geometry::default();
        add(&mut mesh, Sprig::Grass, Vec3::ZERO, 0.0, 1.0, 0.5, 0.0, 1.0);

        let faces: Vec<[u32; 3]> = mesh
            .indices
            .chunks(3)
            .map(|face| [face[0], face[1], face[2]])
            .collect();
        assert!(faces.len() >= 3, "a tuft should have blades");

        for face in &faces {
            let back = [face[0], face[2], face[1]];
            assert!(
                faces.contains(&back),
                "the triangle {face:?} is only wound one way"
            );
        }
    }

    #[test]
    fn the_scatter_keeps_clear_of_the_forest_salts() {
        // Sharing a salt with the woods would put a tuft at the foot of every
        // tree and nowhere else — the two lattices would march in step.
        let ours = [
            SALT_JITTER_X,
            SALT_JITTER_Z,
            SALT_PRESENT,
            SALT_KIND,
            SALT_TURN,
            SALT_SCALE,
            SALT_SHADE,
            SALT_PETAL,
        ];
        for salt in ours {
            assert!(salt > 7, "salt {salt} collides with the forest's");
        }
        let unique: std::collections::HashSet<u32> = ours.into_iter().collect();
        assert_eq!(unique.len(), ours.len(), "two purposes share a salt");
    }
}
