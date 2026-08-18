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
//! # A region NAMES a country; it does not describe a climate
//!
//! This began as two physical fields — how dry, how cold — with the biome
//! inferred from them by threshold. That is how a simulation does it, and it was
//! a steady source of bugs in a game that is not one: the moisture ramp and the
//! treeline and the snowline all pushed each other about, so lowering the snow to
//! reach the coast closed the bare-rock band, and widening the desert to reach a
//! town squeezed out the grassland behind it. Every one of those was a
//! consequence nobody asked for, arrived at by arithmetic from two numbers that
//! nobody wanted to think in.
//!
//! This is a fantasy game about raising monsters. Nobody needs a humidity model
//! to say "the northern desert". So a region simply IS a country: desert, snow,
//! or the ordinary green world. The map says which, and that is the end of it.
//!
//! What is left to height and slope is what height and slope genuinely decide —
//! where the snow line sits on a mountain, which faces are too steep to hold
//! soil. And the map image, the coast and the towns still overrule everything
//! here: a desert with a river through it still has a river through it.

use glam::Vec2;

/// One region of the map, in normalised map coordinates.
///
/// `0,0` is the north-west corner and `1,1` the south-east, so a zone can be read
/// straight off a picture of the world without knowing how many metres across it
/// happens to be.
/// What kind of country somewhere is.
///
/// Three, and deliberately few. Each one is a place a player can name and a place
/// a species of monster can come from, which is the entire job.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Country {
    /// The green world: grass, woods, hills. Most of the map, and where the game
    /// begins.
    #[default]
    Ordinary,
    /// Sand and stone. Nothing grows that does not have to.
    Desert,
    /// Snow above, conifers below, and rock wherever it is too steep for either.
    Snow,
}

impl Country {
    pub fn name(self) -> &'static str {
        match self {
            Country::Ordinary => "ordinary",
            Country::Desert => "desert",
            Country::Snow => "snow",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Zone {
    /// Where its middle sits.
    pub at: Vec2,
    /// How far it reaches, as half-extents. An ellipse rather than a circle,
    /// because a landmass is not round and neither is a climate.
    pub reach: Vec2,
    /// What country this is.
    pub country: Country,
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
        at: Vec2::new(0.42, 0.33),
        reach: Vec2::new(0.19, 0.37),
        country: Country::Desert,
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
        country: Country::Snow,
        edge: 0.32,
    },
    // Its southern shoulder, so the cold does not stop in a circle around the
    // peak. Moved off the desert's new eastern edge — a place that is both
    // parched and frozen is a cold desert, which is a real thing and not one
    // this world has been asked for.
    Zone {
        at: Vec2::new(0.83, 0.62),
        reach: Vec2::new(0.13, 0.24),
        country: Country::Snow,
        edge: 0.5,
    },
];

/// What country a point on the map is in, and how firmly it belongs to it.
///
/// The strength is 1 well inside a region and falls to 0 at its rim, which is
/// what lets ground cover thin out toward a boundary instead of stopping along a
/// line. Everywhere no zone claims comes out [`Country::Ordinary`] at full
/// strength: the green world is the default rather than a fourth zone, because
/// it is what the map is mostly made of and the exceptions are what get named.
pub fn at(uv: Vec2) -> (Country, f32) {
    let mut claimed = Country::Ordinary;
    let mut strongest = 0.0_f32;

    for zone in ZONES {
        // Elliptical distance: 0 at the middle, 1 at the rim.
        let away = ((uv - zone.at) / zone.reach).length();
        if away >= 1.0 {
            continue;
        }
        // Full strength through the middle, easing off across the outer band.
        let strength = crate::smoothstep(1.0, 1.0 - zone.edge.clamp(0.05, 1.0), away);

        // The zone that claims this point most strongly wins outright, rather
        // than the two of them being mixed. A place is one country or another;
        // half a desert and half a snowfield is not a third thing, it is a bug
        // with a plausible-looking number attached.
        if strength > strongest {
            strongest = strength;
            claimed = zone.country;
        }
    }

    if strongest <= 0.0 {
        (Country::Ordinary, 1.0)
    } else {
        (claimed, strongest)
    }
}

/// Which country actually holds a point, with the boundary broken up.
///
/// A country is a hard choice, and a hard choice drawn straight across the map is
/// a LINE — grass on one side, snow on the other, and nothing in between, which
/// is what it looked like. Real country does not change along a line; it changes
/// across a band where the two interlock.
///
/// So how firmly somewhere belongs to its region is raced against a local noise
/// value. Deep inside, belonging wins everywhere and the region is solid; out at
/// the rim it wins only where the noise happens to be low, so the boundary breaks
/// into fingers of one country reaching into the other.
///
/// **Every path that cares which country somewhere is in has to call this**, and
/// that is the whole reason it is here rather than inside the classifier. What a
/// place IS and what it LOOKS like are decided in different files, and the last
/// three times they were given the chance to answer a question separately they
/// answered it differently.
pub fn holding(country: Country, belonging: f32, wooded: f32) -> Country {
    if belonging > wooded * EDGE_DITHER {
        country
    } else {
        Country::Ordinary
    }
}

/// How hard a region's rim has to fight the local noise to hold its ground.
///
/// The number that turns a boundary from a line into a band of interlocking
/// fingers. Bigger means the region gives way sooner and the fringe is wider.
const EDGE_DITHER: f32 = 0.85;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_zone_claims_its_own_middle() {
        // A region nobody can stand in the middle of is not a region.
        for (index, zone) in ZONES.iter().enumerate() {
            let (country, strength) = at(zone.at);
            assert_eq!(country, zone.country, "zone {index} lost its own middle");
            assert!(strength > 0.65, "zone {index} only reaches {strength:.2} there");
        }
    }

    #[test]
    fn the_world_is_mostly_ordinary_country() {
        // The exceptions are what get named. If desert and snow between them
        // covered most of the map, the green world would be the exception — and
        // the green world is where the game starts.
        //
        // Measured over the whole rectangle, most of which is ocean, so the share
        // here runs higher than the share of LAND. What it is really guarding is
        // that nobody quietly grows a zone until it owns the map.
        let mut claimed = 0;
        let mut looked = 0;
        for down in 0..60 {
            for across in 0..60 {
                let uv = Vec2::new(across as f32 / 59.0, down as f32 / 59.0);
                if at(uv).0 != Country::Ordinary {
                    claimed += 1;
                }
                looked += 1;
            }
        }
        let share = claimed as f32 / looked as f32;
        assert!(
            (0.08..0.52).contains(&share),
            "the named regions cover {:.0}% of the map",
            share * 100.0
        );
    }

    #[test]
    fn regions_belong_gradually_rather_than_at_a_line() {
        // How firmly somewhere belongs to its region is what lets cover thin out
        // toward a boundary. If it went one to nought at a line, so would the
        // grass.
        let desert = ZONES[0];
        let mut fading = 0;
        for step in 0..80 {
            let uv = desert.at + Vec2::new(-desert.reach.x * step as f32 / 40.0, 0.0);
            let (country, strength) = at(uv);
            if country == Country::Desert && strength > 0.05 && strength < 0.95 {
                fading += 1;
            }
        }
        assert!(fading > 8, "the desert's edge is only {fading} samples wide");
    }

    #[test]
    fn the_world_reads_west_to_east_as_it_was_drawn() {
        // Green, desert, green, snow. The arrangement the map was drawn with, and
        // the thing every separate tweak to a zone is capable of quietly undoing:
        // two regions grown toward each other squeeze out whatever was between
        // them, and nobody notices until the desert runs into the snow.
        //
        // Read along the middle of the map, west to east.
        let mut bands: Vec<Country> = Vec::new();
        for step in 0..=200 {
            let uv = Vec2::new(step as f32 / 200.0, 0.34);
            let (country, strength) = at(uv);
            // Only where a region has properly taken hold, so a rim does not
            // count as a band of its own.
            if strength < 0.6 {
                continue;
            }
            if bands.last() != Some(&country) {
                bands.push(country);
            }
        }
        assert_eq!(
            bands,
            vec![
                Country::Ordinary,
                Country::Desert,
                Country::Ordinary,
                Country::Snow
            ],
            "the world reads {bands:?} from west to east"
        );
    }
}
