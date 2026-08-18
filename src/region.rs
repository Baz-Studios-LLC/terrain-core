//! Which part of the world you are in, and what that part of the world is like.
//!
//! # Biomes were scattered, and they should be places
//!
//! What kind of ground a point carried used to be decided entirely by that point:
//! a moisture field said dry here and wet there, and desert appeared wherever the
//! noise happened to dip. The result was correct at every point and wrong
//! everywhere — patches of desert in the middle of grassland, a stripe of wood
//! across a dune, no two hundred metres of the map the same as the next.
//!
//! That is not what a world looks like and it is not what a world is FOR. A
//! player says "the northern desert" or "the snow country in the east"; they
//! cannot say anything at all about a place whose character changes every time
//! they walk a field's width. And monsters are supposed to live in these — a
//! species that belongs to the desert needs a desert to belong to, not a scatter
//! of dry patches.
//!
//! So the map is divided into a few large regions with soft edges, and the local
//! noise is demoted to what it should always have been: variation WITHIN a place
//! rather than the thing that decides which place it is.
//!
//! # It does not decide the ground, only its character
//!
//! A region says how dry and how cold somewhere is. It does not say where the
//! hills are, where the coast runs, or where a town has levelled its ground —
//! those come from the map image and from settlement, and they still overrule
//! everything here. A desert with a river through it still has a river through it.

use glam::Vec2;

/// One region of the map, in normalised map coordinates.
///
/// `0,0` is the north-west corner and `1,1` the south-east, so a zone can be read
/// straight off a picture of the world without knowing how many metres across it
/// happens to be.
#[derive(Clone, Copy, Debug)]
pub struct Zone {
    /// Where its middle sits.
    pub at: Vec2,
    /// How far it reaches, as half-extents. An ellipse rather than a circle,
    /// because a landmass is not round and neither is a climate.
    pub reach: Vec2,
    /// How parched it is at its middle, 0 to 1.
    pub arid: f32,
    /// How cold it is at its middle, 0 to 1.
    pub chill: f32,
    /// How far in from the rim it takes to reach full strength, as a share of the
    /// reach. Wide, so one region becomes the next across a day's walk rather
    /// than along a line you could stand on.
    pub edge: f32,
}

/// The regions of this world.
///
/// # What is BETWEEN them is a place too
///
/// Both of these were grown outward, separately, each to cover the ground it had
/// been drawn over — and between them ran a band of grass and wood that neither
/// was thinking about. Two regions expanding toward each other squeezed it out,
/// and the first anyone knew of it was that the desert now ran straight into the
/// snow. Growing a region is not free: it is taken from whatever was there.
///
/// So they are held apart deliberately, and the corridor between them is checked
/// for rather than hoped for.
///
/// Hand-placed rather than grown from noise, and that is the point: these are
/// decisions about what the world IS, and a world whose geography is an accident
/// of a seed cannot be designed around. They are read off the map — a picture of
/// the continents with the areas drawn on it — which is why they are in
/// normalised coordinates.
pub const ZONES: [Zone; 3] = [
    // The northern desert: the whole middle landmass down to where the southern
    // grassland begins, kept clear of the west coast so the grass runs to the sea.
    //
    // Grown southward twice. What is drawn on a map is the area a region should
    // COVER, and a zone's rim is not its region's rim — the falloff means the
    // outer band comes out merely dry rather than parched, so the desert lands
    // well inside the ellipse that produced it. Measure the world, not the zone.
    Zone {
        at: Vec2::new(0.44, 0.33),
        reach: Vec2::new(0.20, 0.37),
        arid: 1.0,
        chill: 0.0,
        edge: 0.55,
    },
    // The snow country: the WHOLE eastern island, not a ring around the peak.
    //
    // Reaching past the island's far coast on purpose. A zone's rim is where its
    // strength runs out, so a zone that merely covers the land leaves that land's
    // edges half-hearted — and half-hearted cold is a forest. The ground behind
    // the mountain has to be as cold as the ground in front of it.
    Zone {
        at: Vec2::new(0.90, 0.40),
        reach: Vec2::new(0.19, 0.50),
        arid: 0.0,
        chill: 1.0,
        edge: 0.32,
    },
    // Its southern shoulder, so the cold does not stop in a circle around the
    // peak. Moved off the desert's new eastern edge — a place that is both
    // parched and frozen is a cold desert, which is a real thing and not one
    // this world has been asked for.
    Zone {
        at: Vec2::new(0.83, 0.62),
        reach: Vec2::new(0.13, 0.24),
        arid: 0.0,
        chill: 0.9,
        edge: 0.5,
    },
];

/// How dry and how cold a point on the map is, each 0 to 1.
///
/// Everywhere not claimed by a zone comes out `(0, 0)`: temperate, watered, and
/// the ordinary country the rest of the world is made of. That is deliberately
/// the default rather than a fourth zone — grass and wood are what this world is
/// mostly made of, and the exceptions are the things worth naming.
pub fn at(uv: Vec2) -> (f32, f32) {
    let mut arid = 0.0_f32;
    let mut chill = 0.0_f32;

    for zone in ZONES {
        // Elliptical distance: 0 at the middle, 1 at the rim.
        let away = ((uv - zone.at) / zone.reach).length();
        if away >= 1.0 {
            continue;
        }
        // Full strength through the middle, easing off across the outer band.
        let strength = crate::smoothstep(1.0, 1.0 - zone.edge.clamp(0.05, 1.0), away);

        // The strongest zone rather than the sum of them. Two overlapping cold
        // regions are still one cold region — adding them would make the overlap
        // colder than either, which is how a seam becomes a landmark.
        arid = arid.max(zone.arid * strength);
        chill = chill.max(zone.chill * strength);
    }

    (arid, chill)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_zone_reaches_full_strength_somewhere() {
        // A region nobody can stand in the middle of is not a region.
        for (index, zone) in ZONES.iter().enumerate() {
            let (arid, chill) = at(zone.at);
            let most = arid.max(chill);
            assert!(
                most > 0.65,
                "zone {index} only reaches {most:.2} at its own middle"
            );
        }
    }

    #[test]
    fn the_world_is_mostly_ordinary_country() {
        // The exceptions are what get named. If desert and snow between them
        // covered most of the map, the grassland would be the exception — and the
        // grassland is where the game starts.
        let mut claimed = 0;
        let mut looked = 0;
        for down in 0..60 {
            for across in 0..60 {
                let uv = Vec2::new(across as f32 / 59.0, down as f32 / 59.0);
                let (arid, chill) = at(uv);
                if arid.max(chill) > 0.5 {
                    claimed += 1;
                }
                looked += 1;
            }
        }
        let share = claimed as f32 / looked as f32;
        assert!(
            (0.08..0.45).contains(&share),
            "the special regions cover {:.0}% of the map",
            share * 100.0
        );
    }

    #[test]
    fn regions_change_gradually_rather_than_at_a_line() {
        // A biome boundary you can stand astride is a seam. Walking out of the
        // desert should take a while.
        let desert = ZONES[0];
        let mut steps = 0;
        // Due west from the middle of the desert, out past its rim.
        for step in 0..80 {
            let uv = desert.at + Vec2::new(-desert.reach.x * step as f32 / 40.0, 0.0);
            let (arid, _) = at(uv);
            if arid > 0.05 && arid < 0.95 {
                steps += 1;
            }
        }
        assert!(
            steps > 12,
            "the desert edge is only {steps} samples wide — that is a line"
        );
    }

    #[test]
    fn ordinary_country_survives_between_the_regions() {
        // Two regions grown toward each other squeeze out whatever was between
        // them, and what was between these two was a band of grass and wood.
        // Nobody notices until the desert runs into the snow.
        //
        // Along the line joining their middles there has to be a stretch that
        // belongs to neither.
        let desert = ZONES[0].at;
        let snow = ZONES[1].at;
        let mut between = 0;
        let mut looked = 0;
        for step in 0..=100 {
            let along = step as f32 / 100.0;
            let uv = desert + (snow - desert) * along;
            let (arid, chill) = at(uv);
            looked += 1;
            if arid < 0.25 && chill < 0.25 {
                between += 1;
            }
        }
        assert!(
            between * 8 > looked,
            "only {between} of {looked} steps between the desert and the snow              belong to neither — they have grown into each other"
        );
    }

    #[test]
    fn the_desert_and_the_snow_are_not_the_same_place() {
        // Two regions that overlap are one region with two names.
        let (arid_at_desert, chill_at_desert) = at(ZONES[0].at);
        let (arid_at_snow, chill_at_snow) = at(ZONES[1].at);
        assert!(arid_at_desert > 0.65 && chill_at_desert < 0.2);
        assert!(chill_at_snow > 0.65 && arid_at_snow < 0.2);
    }
}
