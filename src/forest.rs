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


use std::collections::HashMap;

use glam::{Vec2, Vec3};

use crate::history::History;
use crate::smoothstep;

pub use crate::Patch;

/// Names the file, so a stale or unrelated one is refused.
const MAGIC: &[u8; 8] = b"RNGRFST1";

/// Meters per cell of the painted layer. Must match the bench's.
pub const CELL: f32 = 16.0;

/// Below this, a cell is untouched and the ground's answer stands.
const PAINTED_EPSILON: f32 = 0.01;

/// How many strokes of planting can be taken back. The same depth the ground
/// keeps, because a maker pressing the same key expects the same reach.
const UNDO_DEPTH: usize = 64;

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
    history: History,
}

impl std::fmt::Debug for Painted {
    // Its shape and how much is painted, never the whole grid: a world of it is
    // millions of cells and dumping them helps nobody.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Painted {{ {}x{} cells over {:.0}x{:.0} m, {} painted }}",
            self.wide, self.deep, self.half.x * 2.0, self.half.y * 2.0, self.painted
        )
    }
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
            history: History::new(UNDO_DEPTH),
        }
    }

    /// Reads a painted layer from the bytes of a `forest.bin`.
    ///
    /// Takes BYTES, not a path, and returns the reason on failure rather than
    /// logging it. Where the file lives and how a problem is reported are each
    /// program's own business — this crate is linked by two of them and has no
    /// business deciding either.
    pub fn read(bytes: &[u8], half: Vec2) -> Result<Self, String> {
        let empty = Self::empty(half);
        let header = 8 + 4 * 4;
        if bytes.len() < header || &bytes[..8] != MAGIC {
            return Err("not a painted forest".into());
        }

        let word = |at: usize| {
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
        };
        let real =
            |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
        let (wide, deep) = (word(8), word(12));
        let saved_half = Vec2::new(real(16), real(20));

        // Refused rather than stretched. Woods landing in the wrong places is
        // worse than none, and nothing on screen would say why.
        if wide != empty.wide || deep != empty.deep || saved_half.distance(half) > 1.0 {
            return Err(format!(
                "painted for a {:.0}x{:.0} m world, not this {:.0}x{:.0} m one",
                saved_half.x * 2.0,
                saved_half.y * 2.0,
                half.x * 2.0,
                half.y * 2.0
            ));
        }
        if bytes.len() < header + wide * deep * 4 {
            return Err("truncated".into());
        }

        let bias: Vec<f32> = (0..wide * deep).map(|i| real(header + i * 4)).collect();
        let painted = bias.iter().filter(|v| v.abs() > PAINTED_EPSILON).count();
        Ok(Self { bias, painted, ..empty })
    }

    /// Writes the layer out, for whichever program is allowed to plant.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + 16 + self.bias.len() * 4);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&(self.wide as u32).to_le_bytes());
        out.extend_from_slice(&(self.deep as u32).to_le_bytes());
        out.extend_from_slice(&self.half.x.to_le_bytes());
        out.extend_from_slice(&self.half.y.to_le_bytes());
        for value in &self.bias {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }

    /// Paints, positive to plant and negative to clear. Returns the ground it
    /// changed, so the trees standing there can be grown again.
    ///
    /// It lives here because a wood painted at the bench and a wood painted in
    /// the game must be the same wood, and two implementations of the falloff
    /// would not be.
    pub fn paint(&mut self, centre: Vec2, radius: f32, amount: f32) -> Patch {
        let to_cell = |v: f32, half: f32, count: usize| {
            (((v + half) / CELL).floor() as isize).clamp(0, count as isize - 1) as usize
        };
        let x0 = to_cell(centre.x - radius, self.half.x, self.wide);
        let x1 = to_cell(centre.x + radius + CELL, self.half.x, self.wide);
        let z0 = to_cell(centre.y - radius, self.half.y, self.deep);
        let z1 = to_cell(centre.y + radius + CELL, self.half.y, self.deep);

        for z in z0..=z1 {
            for x in x0..=x1 {
                let at = Vec2::new(
                    x as f32 * CELL - self.half.x,
                    z as f32 * CELL - self.half.y,
                );
                let away = at.distance(centre);
                if away > radius {
                    continue;
                }
                let falloff = smoothstep(radius, 0.0, away);
                let cell = z * self.wide + x;
                let now = (self.bias[cell] + amount * falloff).clamp(-1.0, 1.0);
                self.history.record(cell, self.bias[cell]);
                self.write(cell, now);
            }
        }
        (
            centre - Vec2::splat(radius + CELL),
            centre + Vec2::splat(radius + CELL),
        )
    }

    // ------------------------------------------------------------ taking back

    /// Opens an undo group, exactly as the ground's does — one press takes back
    /// a whole drag rather than the last frame of one.
    ///
    /// Planting shipped without this, and `Ctrl+Z` after growing a wood either
    /// did nothing or took back a hillside from ten minutes earlier. Clearing
    /// what you planted is not the same thing: it leaves the cells written, so
    /// the ground's own answer never comes back.
    pub fn begin_stroke(&mut self) {
        self.history.begin();
    }

    pub fn end_stroke(&mut self) {
        self.history.end();
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Takes back the last stroke of planting, and says what ground changed so
    /// the trees standing there can be grown again.
    pub fn undo(&mut self) -> Option<Patch> {
        let stroke = self.history.take_undo()?;
        let ground = self.ground_of(&stroke);
        let inverse = self.put_back(&stroke);
        self.history.push_redo(inverse);
        Some(ground)
    }

    pub fn redo(&mut self) -> Option<Patch> {
        let stroke = self.history.take_redo()?;
        let ground = self.ground_of(&stroke);
        let inverse = self.put_back(&stroke);
        self.history.push_undo(inverse);
        Some(ground)
    }

    /// Writes saved values back, returning what was there so it can be reversed
    /// again.
    fn put_back(&mut self, values: &HashMap<usize, f32>) -> HashMap<usize, f32> {
        let mut inverse = HashMap::with_capacity(values.len());
        for (&cell, &value) in values {
            inverse.insert(cell, self.bias[cell]);
            self.write(cell, value);
        }
        inverse
    }

    /// The ground a set of cells covers, padded by one cell because reading is
    /// bilinear and reaches a cell past whatever was written.
    fn ground_of(&self, values: &HashMap<usize, f32>) -> Patch {
        let mut low = Vec2::splat(f32::MAX);
        let mut high = Vec2::splat(f32::MIN);
        for &cell in values.keys() {
            let at = Vec2::new(
                (cell % self.wide) as f32 * CELL - self.half.x,
                (cell / self.wide) as f32 * CELL - self.half.y,
            );
            low = low.min(at);
            high = high.max(at);
        }
        (low - CELL, high + CELL)
    }

    /// Writes one cell, keeping the painted count in step.
    fn write(&mut self, cell: usize, value: f32) {
        let was = self.bias[cell].abs() > PAINTED_EPSILON;
        let is = value.abs() > PAINTED_EPSILON;
        match (was, is) {
            (false, true) => self.painted += 1,
            (true, false) => self.painted -= 1,
            _ => {}
        }
        self.bias[cell] = value;
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

#[cfg(test)]
mod round_trip {
    use super::*;

    const HALF: Vec2 = Vec2::new(800.0, 600.0);

    #[test]
    fn painting_survives_being_written_and_read() {
        let mut painted = Painted::empty(HALF);
        painted.paint(Vec2::new(100.0, -50.0), 80.0, 1.0);
        assert!(painted.at(100.0, -50.0) > 0.9, "the middle should be planted");

        let read = Painted::read(&painted.to_bytes(), HALF).expect("should read back");
        assert_eq!(read.painted_cells(), painted.painted_cells());
        assert!((read.at(100.0, -50.0) - painted.at(100.0, -50.0)).abs() < 1.0e-5);
    }

    #[test]
    fn a_layer_from_another_world_is_refused_with_a_reason() {
        // Silence here would put woods in the wrong places with nothing to say
        // why, so the reason is the point.
        let painted = Painted::empty(HALF);
        let why = Painted::read(&painted.to_bytes(), HALF * 2.0).unwrap_err();
        assert!(why.contains("world"), "unhelpful reason: {why}");

        assert!(Painted::read(b"not a forest at all", HALF).is_err());
        let mut short = painted.to_bytes();
        short.truncate(40);
        assert_eq!(Painted::read(&short, HALF).unwrap_err(), "truncated");
    }

    #[test]
    fn one_undo_takes_back_a_whole_drag_of_planting() {
        // Planting shipped with no history at all, so Ctrl+Z after growing a
        // wood either did nothing or took back a hillside from ten minutes
        // earlier. A drag is many ticks and has to come back in one press.
        let mut painted = Painted::empty(HALF);

        painted.begin_stroke();
        for i in 0..20 {
            painted.paint(Vec2::new(i as f32 * 8.0, 0.0), 40.0, 0.2);
        }
        painted.end_stroke();

        let planted = painted.at(80.0, 0.0);
        assert!(planted > 0.5, "the drag should have planted: {planted}");
        assert!(painted.can_undo());

        painted.undo().expect("undo says what changed");
        assert_eq!(
            painted.painted_cells(),
            0,
            "the woods should return to exactly what the ground alone said"
        );

        painted.redo().expect("redo says what changed");
        assert!(
            (painted.at(80.0, 0.0) - planted).abs() < 1.0e-4,
            "redo should restore the drag exactly"
        );
    }

    #[test]
    fn undoing_is_not_the_same_as_clearing() {
        // Clearing WRITES negative bias: it forces bare ground, and holds it
        // bare against whatever the ground itself would have said. That is a
        // decision, and it stays made. Undo is the only way back to zero — to
        // no decision at all — which is the whole meaning of the layer.
        //
        // Paint and clear by equal amounts do cancel to zero in the middle of a
        // stroke, which is what made this look interchangeable. Away from that
        // exact case they are nothing alike: clear ground nobody planted and the
        // bias goes negative and stays there.
        let mut cleared = Painted::empty(HALF);
        cleared.paint(Vec2::ZERO, 60.0, -1.0);
        assert!(
            cleared.at(0.0, 0.0) < -PAINTED_EPSILON,
            "clearing should hold the ground bare, not fall back to it: {}",
            cleared.at(0.0, 0.0)
        );
        assert!(cleared.painted_cells() > 0, "and it counts as painted");

        let mut undone = Painted::empty(HALF);
        undone.begin_stroke();
        undone.paint(Vec2::ZERO, 60.0, 1.0);
        undone.end_stroke();
        undone.undo();
        assert_eq!(undone.at(0.0, 0.0), 0.0, "undo should leave nothing behind");
        assert_eq!(undone.painted_cells(), 0);
    }

    #[test]
    fn clearing_takes_back_what_planting_put_down() {
        let mut painted = Painted::empty(HALF);
        painted.paint(Vec2::ZERO, 60.0, 1.0);
        painted.paint(Vec2::ZERO, 60.0, -1.0);
        assert!(painted.at(0.0, 0.0).abs() < PAINTED_EPSILON);
        assert_eq!(painted.painted_cells(), 0, "no cell left counted as painted");
    }
}
