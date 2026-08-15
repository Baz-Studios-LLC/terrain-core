//! The ground a maker put there.
//!
//! Generated terrain gets you a plausible landscape; it does not get you THIS
//! hill, HERE. This is where authored geography lives: a grid of signed height
//! offsets in metres, laid over whatever the map and the noise produced, and
//! sculpted with a brush.
//!
//! # Offsets, not heights
//!
//! Every cell holds how far the ground moved, not where the ground is. That one
//! choice is what lets the two coexist. Re-roll the noise, swap the map image
//! for a redrawn one, change the world's size — and a hand-placed hill stays a
//! hill, riding on top of the new ground instead of being overwritten by it. A
//! grid of absolute heights would mean a maker's whole afternoon is invalidated
//! by the game's next tuning pass, and nobody would sculpt anything.
//!
//! # Both programs sculpt now
//!
//! This began as Opificium's alone, because the bench was where shaping
//! happened and the game only read the file. That has changed: the game carries
//! its own sculpting mode, the way a studio's editor is built on top of its
//! runtime rather than beside it. So the brush lives here, once, and the two
//! cannot drift apart — which is the same reason [`crate::forest`] does.
//!
//! It knows nothing about either of them. No bench, no window, no asset folder:
//! a grid, a brush, and bytes. Where the file lives and what to say when it is
//! wrong are each program's own business.

use std::collections::HashMap;

use glam::{Vec2, Vec3};
use noise::{NoiseFn, Perlin};

use crate::history::History;
use crate::smoothstep;

pub use crate::Patch;

/// Names the file, so a stale or unrelated one is refused rather than read as
/// garbage elevation.
const MAGIC: &[u8; 8] = b"RNGREDT1";

/// Below this, an offset is untouched ground.
const SCULPT_EPSILON: f32 = 0.01;

/// Metres per cell of the edit grid. Fine enough to shape one hill, coarse
/// enough that a world's worth of it is a few megabytes.
pub const CELL: f32 = 4.0;

/// How many strokes can be taken back. Each holds only the cells it touched, so
/// this bounds memory by ground painted rather than by the size of the world.
const UNDO_DEPTH: usize = 64;

/// Spatial frequency of the roughening brush, in cycles per metre.
const ROUGHEN_FREQ: f64 = 0.05;

/// The angle of repose, as a rise over the run between two neighbouring cells.
///
/// Loose material will not hold a slope steeper than this — it slides. Around
/// 34 degrees is what sand, scree and most soils settle at, and it is why real
/// hillsides look the way they do rather than like the noise that made them.
const REPOSE: f32 = 0.67;

/// How much of the excess above the repose angle moves in one tick. Low, so
/// erosion is something you hold the brush on and watch happen rather than a
/// switch that ruins a hill in one frame.
const SLUMP_RATE: f32 = 0.35;

/// Blend fraction per second for the tools that converge on a target rather
/// than pushing at a fixed speed.
const BLEND_RATE: f32 = 4.0;

/// The middle of the strength range, which the tools measured as a speed are
/// tuned against.
pub const MIDDLING_STRENGTH: f32 = 25.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Brushing {
    Raise,
    Lower,
    /// Toward the average height nearby.
    Smooth,
    /// To a fixed height, with a soft dish.
    Flatten,
    /// A dirt road: the ground graded along its run and worn to bare earth.
    ///
    /// It used to level to one height with a flat bed, which on any slope cuts
    /// a trench with shoulders — a cutting, not a road. A road FOLLOWS the land;
    /// it takes the bumps out so a cart can pass and leaves the hill a hill.
    /// What makes it read as a road rather than as smoothed grass is the
    /// surface, which the tool paints as well as grading.
    Path,
    /// Fractal detail, for ground that has been sculpted too smooth.
    Roughen,
    /// Let the ground slump: anything steeper than a slope material will hold
    /// slides downhill and piles up at the foot.
    Erode,
    /// A graded run between two points, laid in one go rather than painted.
    Ramp,
    /// Plant trees, or take them away. Touches the woods, never the ground.
    Plant,
}

impl Brushing {
    pub const ALL: [Brushing; 9] = [
        Brushing::Raise,
        Brushing::Lower,
        Brushing::Smooth,
        Brushing::Flatten,
        Brushing::Path,
        Brushing::Roughen,
        Brushing::Erode,
        Brushing::Ramp,
        Brushing::Plant,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Brushing::Raise => "RAISE",
            Brushing::Lower => "LOWER",
            Brushing::Smooth => "SMOOTH",
            Brushing::Flatten => "FLATTEN",
            Brushing::Path => "PATH",
            Brushing::Roughen => "ROUGHEN",
            Brushing::Erode => "ERODE",
            Brushing::Ramp => "RAMP",
            Brushing::Plant => "PLANT",
        }
    }

    pub fn said(self) -> &'static str {
        match self {
            Brushing::Raise => "Push the ground up",
            Brushing::Lower => "Pull the ground down",
            Brushing::Smooth => "Average out what is there",
            Brushing::Flatten => "Level to where you pressed",
            Brushing::Path => "A dirt road, graded to follow the land",
            Brushing::Roughen => "Break up ground sculpted too smooth",
            Brushing::Erode => "Let steep ground slump and settle",
            Brushing::Ramp => "Click two points for a graded run",
            Brushing::Plant => "Plant trees, right button clears them",
        }
    }

    /// Whether this is laid between two clicked points rather than painted by
    /// dragging. Both programs take a different gesture for these.
    pub fn is_two_point(self) -> bool {
        matches!(self, Brushing::Ramp)
    }

    /// How far either side a levelling tool averages, in cells.
    ///
    /// SMOOTH works locally — it is for taking the edge off one spike. A road is
    /// graded over a longer run, and it has to be: a three-cell average spans
    /// twelve metres, which barely touches a bump nine metres across, so PATH
    /// left the ground almost as rough as it found it and read as a lawn rather
    /// than a road.
    fn grading(self) -> isize {
        match self {
            Brushing::Path => 3,
            _ => 1,
        }
    }

    /// Whether this wears the ground down to bare earth as it works.
    ///
    /// Grading alone leaves a smooth strip of grass, which is a lawn. What makes
    /// a road is that it is WORN — so the tool paints the surface layer as well,
    /// and the two together are the road.
    pub fn is_surfacing(self) -> bool {
        matches!(self, Brushing::Path)
    }

    /// Whether this touches the woods rather than the ground.
    ///
    /// Asked before a stroke reaches the grid at all: planting writes to the
    /// forest's own painted layer, and a brush that moved earth as well would
    /// dig a hole every time somebody grew a wood.
    pub fn is_planting(self) -> bool {
        matches!(self, Brushing::Plant)
    }

    /// How much of itself a tool applies in one tick.
    ///
    /// The strength control means a different thing to each kind of tool, and
    /// this is the one place that decides what. Kept here rather than at the
    /// call site so that adding a tool cannot forget to answer the question —
    /// erosion did exactly that, and its strength control moved nothing.
    pub fn rate(self, strength: f32, delta: f32) -> f32 {
        match self {
            // Metres of vertical push per second.
            Brushing::Raise | Brushing::Lower | Brushing::Roughen => strength * delta,
            // Erosion is a SPEED, not a distance: strength decides how fast the
            // ground settles, never how far it slides — where it comes to rest
            // is the angle of repose's business and not the maker's. Scaled
            // against the middle of the strength range, so the middle of the
            // slider is the pace the tool was tuned at.
            Brushing::Erode => strength / MIDDLING_STRENGTH * delta,
            // Bias per second. Strength is how quickly a wood thickens under
            // the brush, so a light touch thins a stand rather than clearing it.
            Brushing::Plant => strength / MIDDLING_STRENGTH * delta,
            // The rest converge on a target, so this is a blend fraction — how
            // much of the remaining distance to close this tick. Scaled by
            // strength like everything else: a control that moves nothing for
            // half the tools on the shelf is worse than no control, and
            // levelling wants a gentle setting for easing ground over and a
            // firm one for cutting a terrace.
            _ => BLEND_RATE * (strength / MIDDLING_STRENGTH) * delta,
        }
    }

    /// From the middle of the brush out to its rim.
    fn falloff(self, distance: f32, radius: f32) -> f32 {
        match self {
            // A flat bed to seven tenths of the way out, then quick shoulders.
            // The difference between a road cut and a soft dish.
            Brushing::Path => smoothstep(radius, radius * 0.7, distance),
            _ => smoothstep(radius, 0.0, distance),
        }
    }
}

/// One tick of a stroke.
pub struct Stamp<'a> {
    pub centre: Vec2,
    pub radius: f32,
    pub how: Brushing,
    /// Metres this tick for the directional tools, a blend fraction for the rest.
    pub amount: f32,
    /// The height the levelling tools pull toward.
    pub target: f32,
    /// The GENERATED height at a point, with edits left out.
    ///
    /// Smooth, Flatten and Path all work on the finished surface, so the offset
    /// they want to write depends on what the ground was doing underneath. It
    /// must not consult the edit layer: this runs while the caller holds the
    /// lock over it, and asking would deadlock against itself.
    pub under: &'a dyn Fn(Vec2) -> f32,
}

pub struct Sculpt {
    wide: usize,
    deep: usize,
    half: Vec2,
    /// Signed offset in metres, row-major, north row first.
    offsets: Vec<f32>,
    /// Running count of cells moved off zero. Kept as cells are written rather
    /// than counted on demand: the shelf asks every frame and the grid is over
    /// two million cells.
    sculpted: usize,
    pub unsaved: bool,

    history: History,
    noise: Perlin,
}

impl std::fmt::Debug for Sculpt {
    // Its shape and how much is sculpted, never the whole grid: a world of it is
    // millions of cells and dumping them helps nobody.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Sculpt {{ {}x{} cells over {:.0}x{:.0} m, {} sculpted{} }}",
            self.wide,
            self.deep,
            self.half.x * 2.0,
            self.half.y * 2.0,
            self.sculpted,
            if self.unsaved { ", unsaved" } else { "" }
        )
    }
}

impl Sculpt {
    /// An empty layer: the world exactly as generated.
    pub fn empty(half: Vec2, seed: u32) -> Self {
        let wide = (half.x * 2.0 / CELL).ceil() as usize + 1;
        let deep = (half.y * 2.0 / CELL).ceil() as usize + 1;
        Self {
            wide,
            deep,
            half,
            offsets: vec![0.0; wide * deep],
            sculpted: 0,
            unsaved: false,
            history: History::new(UNDO_DEPTH),
            noise: Perlin::new(seed),
        }
    }

    // --------------------------------------------------------------- the bytes

    /// Reads sculpted ground from the bytes of an `edits.bin`.
    ///
    /// Takes BYTES, not a path, and returns the reason on failure rather than
    /// logging it — the same bargain [`crate::forest::Painted::read`] makes, and
    /// for the same reason: this crate is linked by two programs and has no
    /// business deciding where either keeps its files or how it reports trouble.
    pub fn read(bytes: &[u8], half: Vec2, seed: u32) -> Result<Self, String> {
        let empty = Self::empty(half, seed);
        let header = 8 + 4 * 4;
        if bytes.len() < header || &bytes[..8] != MAGIC {
            return Err("not sculpted ground".into());
        }

        let word = |at: usize| {
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
        };
        let real =
            |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
        let (wide, deep) = (word(8), word(12));
        let kept_half = Vec2::new(real(16), real(20));

        // Refused rather than stretched. Offsets that landed in the wrong places
        // would be worse than none: a maker would see their afternoon's work
        // smeared across the map and have nothing to undo it with.
        if wide != empty.wide || deep != empty.deep || kept_half.distance(half) > 1.0 {
            return Err(format!(
                "sculpted for a {:.0}x{:.0} m world, not this {:.0}x{:.0} m one",
                kept_half.x * 2.0,
                kept_half.y * 2.0,
                half.x * 2.0,
                half.y * 2.0
            ));
        }
        if bytes.len() < header + wide * deep * 4 {
            return Err("cut short".into());
        }

        let offsets: Vec<f32> = (0..wide * deep).map(|i| real(header + i * 4)).collect();
        // Counted once here; `write_plainly` keeps it current from then on.
        let sculpted = offsets.iter().filter(|v| v.abs() > SCULPT_EPSILON).count();
        Ok(Self {
            offsets,
            sculpted,
            ..empty
        })
    }

    /// Writes the layer out, for whichever program is doing the saving.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + 16 + self.offsets.len() * 4);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&(self.wide as u32).to_le_bytes());
        out.extend_from_slice(&(self.deep as u32).to_le_bytes());
        out.extend_from_slice(&self.half.x.to_le_bytes());
        out.extend_from_slice(&self.half.y.to_le_bytes());
        for offset in &self.offsets {
            out.extend_from_slice(&offset.to_le_bytes());
        }
        out
    }

    /// Says the bytes reached a file. Separate from [`Self::to_bytes`] because
    /// only the caller knows whether the write actually landed.
    pub fn mark_saved(&mut self) {
        self.unsaved = false;
    }

    // -------------------------------------------------------------- questions

    pub fn sculpted_cells(&self) -> usize {
        self.sculpted
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// The offset at a world position, read between cells. Off the grid reads as
    /// zero, so the open sea past the world's edge is never sculpted by accident.
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

        let cell = |x: usize, z: usize| self.offsets[z * self.wide + x];
        let near = cell(x0, z0) * (1.0 - tx) + cell(x1, z0) * tx;
        let far = cell(x0, z1) * (1.0 - tx) + cell(x1, z1) * tx;
        near * (1.0 - tz) + far * tz
    }

    // ------------------------------------------------------------ taking back

    /// Opens an undo group. Everything written until `end_stroke` comes back in
    /// one press, so a drag lasting two hundred frames is one undo and not two
    /// hundred.
    pub fn begin_stroke(&mut self) {
        self.history.begin();
    }

    pub fn end_stroke(&mut self) {
        self.history.end();
    }

    /// Takes back the last stroke, and says what ground changed so the caller
    /// knows which chunks to mesh again.
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
            inverse.insert(cell, self.offsets[cell]);
            self.write_plainly(cell, value);
        }
        self.unsaved = true;
        inverse
    }

    /// The ground a set of cells covers, padded by one cell because reading is
    /// bilinear and reaches a cell past whatever was written.
    fn ground_of(&self, values: &HashMap<usize, f32>) -> Patch {
        let mut low = Vec2::splat(f32::MAX);
        let mut high = Vec2::splat(f32::MIN);
        for &cell in values.keys() {
            let at = self.cell_at(cell % self.wide, cell / self.wide);
            low = low.min(at);
            high = high.max(at);
        }
        (low - CELL, high + CELL)
    }

    // ------------------------------------------------------------- the stroke

    /// Lays one tick of the brush down. Says what ground changed.
    pub fn apply(&mut self, stamp: &Stamp) -> Patch {
        let (x0, x1, z0, z1) = self.cells_under(stamp.centre, stamp.radius);

        // Smoothing reads its neighbours while writing, so it is worked out into
        // a scratch list and laid down afterwards. Otherwise a cell smooths
        // against values already smoothed this tick, and the whole stroke drags
        // in whatever order the loop happened to run.
        let mut afterward: Vec<(usize, f32)> = Vec::new();

        for z in z0..=z1 {
            for x in x0..=x1 {
                let at = self.cell_at(x, z);
                let away = at.distance(stamp.centre);
                if away > stamp.radius {
                    continue;
                }
                let falloff = stamp.how.falloff(away, stamp.radius);
                if falloff <= 0.0 {
                    continue;
                }

                let cell = z * self.wide + x;
                let now = self.offsets[cell];

                match stamp.how {
                    Brushing::Raise => self.write(cell, now + stamp.amount * falloff),
                    Brushing::Lower => self.write(cell, now - stamp.amount * falloff),
                    Brushing::Roughen => {
                        let n = self
                            .noise
                            .get([at.x as f64 * ROUGHEN_FREQ, at.y as f64 * ROUGHEN_FREQ])
                            as f32;
                        self.write(cell, now + n * stamp.amount * falloff);
                    }
                    Brushing::Flatten => {
                        let wanted = stamp.target - (stamp.under)(at);
                        let t = (stamp.amount * falloff).clamp(0.0, 1.0);
                        self.write(cell, now + (wanted - now) * t);
                    }
                    // Toward what is around it, not toward one height. That is
                    // the whole difference between a road and a cutting: a road
                    // loses the bumps and keeps the hill.
                    Brushing::Smooth | Brushing::Path => {
                        let average = self.thereabouts(x, z, stamp.under, stamp.how.grading());
                        let wanted = average - (stamp.under)(at);
                        let t = (stamp.amount * falloff).clamp(0.0, 1.0);
                        afterward.push((cell, now + (wanted - now) * t));
                    }
                    // Handled in a sweep of its own below: erosion MOVES
                    // material between cells rather than setting each from what
                    // it can see, so it cannot be written one cell at a time.
                    Brushing::Erode => {}
                    // Laid between two points, not painted. See `ramp`.
                    Brushing::Ramp => {}
                    // Touches the woods, never the ground. Handled by the
                    // forest's own painted layer, which this grid knows nothing
                    // about — a brush that moved earth as well would make
                    // planting a wood dig a hole.
                    Brushing::Plant => {}
                }
            }
        }

        for (cell, value) in afterward {
            self.write(cell, value);
        }

        if stamp.how == Brushing::Erode {
            self.slump(stamp, x0, x1, z0, z1);
        }

        self.unsaved = true;
        (
            stamp.centre - Vec2::splat(stamp.radius + CELL),
            stamp.centre + Vec2::splat(stamp.radius + CELL),
        )
    }

    /// Thermal erosion: ground steeper than it can hold slides downhill.
    ///
    /// For every cell, the height difference to each neighbour is compared
    /// against the angle of repose. Whatever is steeper than that is excess, and
    /// a share of the excess MOVES — off the high cell and onto the low one.
    /// Nothing is created or destroyed, which is the whole point: a hill does
    /// not shrink, it settles. Ridges round off, faces shed material, and it
    /// piles into scree at the foot the way a real slope does.
    ///
    /// Worked out in full before anything is written. A sweep that applied as it
    /// went would push material across the brush in whatever order the loop
    /// happened to run, which slides the whole hill one way.
    fn slump(&mut self, stamp: &Stamp, x0: usize, x1: usize, z0: usize, z1: usize) {
        let wide = x1 - x0 + 1;
        let deep = z1 - z0 + 1;
        let mut moved = vec![0.0f32; wide * deep];

        // The finished height at a cell: what the generator made, plus what has
        // been sculpted onto it. Erosion works on the surface you can see, not
        // on the offsets underneath it.
        let surface = |grid: &Self, x: usize, z: usize| {
            let at = grid.cell_at(x, z);
            (stamp.under)(at) + grid.offsets[z * grid.wide + x]
        };

        for z in z0..=z1 {
            for x in x0..=x1 {
                let at = self.cell_at(x, z);
                let away = at.distance(stamp.centre);
                if away > stamp.radius {
                    continue;
                }
                let falloff = stamp.how.falloff(away, stamp.radius);
                if falloff <= 0.0 {
                    continue;
                }

                let here = surface(self, x, z);
                // The four square neighbours. Diagonals are left out on purpose:
                // they sit further apart, so including them at the same repose
                // angle biases the slumping along the diagonals.
                for (nx, nz) in [
                    (x.wrapping_sub(1), z),
                    (x + 1, z),
                    (x, z.wrapping_sub(1)),
                    (x, z + 1),
                ] {
                    if nx < x0 || nx > x1 || nz < z0 || nz > z1 {
                        continue;
                    }
                    let drop = here - surface(self, nx, nz);
                    let excess = drop - REPOSE * CELL;
                    if excess <= 0.0 {
                        continue;
                    }
                    // A quarter, because a cell may shed to four neighbours and
                    // must not give away more than it has.
                    let share = excess * 0.25 * SLUMP_RATE * stamp.amount * falloff;
                    moved[(z - z0) * wide + (x - x0)] -= share;
                    moved[(nz - z0) * wide + (nx - x0)] += share;
                }
            }
        }

        for z in z0..=z1 {
            for x in x0..=x1 {
                let shift = moved[(z - z0) * wide + (x - x0)];
                if shift.abs() < 1.0e-5 {
                    continue;
                }
                let cell = z * self.wide + x;
                self.write(cell, self.offsets[cell] + shift);
            }
        }
    }

    /// Lays a graded run between two points, in one go.
    ///
    /// The height climbs steadily from one end to the other, so what comes out
    /// can be walked and carted. This is what the levelling brushes cannot do:
    /// Flatten and Path pull toward ONE height, which is right for a town square
    /// and useless for a road up a hillside.
    ///
    /// Returns the ground it changed.
    pub fn ramp(&mut self, from: Vec3, to: Vec3, width: f32, under: &dyn Fn(Vec2) -> f32) -> Patch {
        let start = Vec2::new(from.x, from.z);
        let end = Vec2::new(to.x, to.z);
        let run = end - start;
        let length = run.length_squared();
        if length < 1.0 {
            return (start, start);
        }

        let reach = width * 2.0;
        let low = start.min(end) - reach;
        let high = start.max(end) + reach;
        let (x0, x1, z0, z1) = self.cells_between(low, high);

        for z in z0..=z1 {
            for x in x0..=x1 {
                let at = self.cell_at(x, z);
                let along = ((at - start).dot(run) / length).clamp(0.0, 1.0);
                let away = start.lerp(end, along).distance(at);
                // A flat bed to the stated width, then shoulders easing back
                // into the land - the same profile the Path brush cuts, because
                // a ramp is a road that happens to climb.
                let pull = smoothstep(reach, width, away);
                if pull <= 0.0 {
                    continue;
                }
                let wanted = from.y + (to.y - from.y) * along;
                let cell = z * self.wide + x;
                let now = self.offsets[cell];
                let offset = wanted - under(at);
                self.write(cell, now + (offset - now) * pull);
            }
        }

        self.unsaved = true;
        (low, high)
    }

    /// The cells covered by a rectangle, kept inside the grid.
    fn cells_between(&self, low: Vec2, high: Vec2) -> (usize, usize, usize, usize) {
        let to_cell = |v: f32, half: f32, count: usize| {
            (((v + half) / CELL).floor() as isize).clamp(0, count as isize - 1) as usize
        };
        (
            to_cell(low.x, self.half.x, self.wide),
            to_cell(high.x + CELL, self.half.x, self.wide),
            to_cell(low.y, self.half.y, self.deep),
            to_cell(high.y + CELL, self.half.y, self.deep),
        )
    }

    /// Writes one cell, remembering what it held for the undo and keeping the
    /// sculpted count in step.
    fn write(&mut self, cell: usize, value: f32) {
        self.history.record(cell, self.offsets[cell]);
        self.write_plainly(cell, value);
    }

    /// Writes without touching the history - used by the undo, which is already
    /// managing the history around the call.
    fn write_plainly(&mut self, cell: usize, value: f32) {
        let was = self.offsets[cell].abs() > SCULPT_EPSILON;
        let is = value.abs() > SCULPT_EPSILON;
        match (was, is) {
            (false, true) => self.sculpted += 1,
            (true, false) => self.sculpted -= 1,
            _ => {}
        }
        self.offsets[cell] = value;
    }

    /// Where a cell sits in the world.
    fn cell_at(&self, x: usize, z: usize) -> Vec2 {
        Vec2::new(x as f32 * CELL - self.half.x, z as f32 * CELL - self.half.y)
    }

    /// The cells a brush covers, kept inside the grid.
    fn cells_under(&self, centre: Vec2, radius: f32) -> (usize, usize, usize, usize) {
        let to_cell = |v: f32, half: f32, count: usize| {
            (((v + half) / CELL).floor() as isize).clamp(0, count as isize - 1) as usize
        };
        (
            to_cell(centre.x - radius, self.half.x, self.wide),
            to_cell(centre.x + radius + CELL, self.half.x, self.wide),
            to_cell(centre.y - radius, self.half.y, self.deep),
            to_cell(centre.y + radius + CELL, self.half.y, self.deep),
        )
    }

    /// The average finished height in the cells around one, out to `reach`.
    fn thereabouts(&self, x: usize, z: usize, under: &dyn Fn(Vec2) -> f32, reach: isize) -> f32 {
        let mut total = 0.0;
        let mut count = 0.0;
        for dz in -reach..=reach {
            for dx in -reach..=reach {
                let nx = (x as isize + dx).clamp(0, self.wide as isize - 1) as usize;
                let nz = (z as isize + dz).clamp(0, self.deep as isize - 1) as usize;
                let at = self.cell_at(nx, nz);
                total += under(at) + self.offsets[nz * self.wide + nx];
                count += 1.0;
            }
        }
        total / count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALF: Vec2 = Vec2::new(400.0, 300.0);
    const SEED: u32 = 7;

    /// The bottom of a bench's strength range.
    const WEAKEST: f32 = 2.0;

    /// Flat ground, so what is under test is the brush and nothing else.
    fn flat(_: Vec2) -> f32 {
        0.0
    }

    fn stamp(centre: Vec2, radius: f32, how: Brushing, amount: f32, target: f32) -> Stamp<'static> {
        Stamp {
            centre,
            radius,
            how,
            amount,
            target,
            under: &flat,
        }
    }

    fn across(patch: Patch) -> Vec2 {
        patch.1 - patch.0
    }

    #[test]
    fn raise_lifts_the_middle_and_fades_to_nothing_at_the_rim() {
        let mut ground = Sculpt::empty(HALF, SEED);
        let centre = Vec2::new(40.0, -20.0);
        let radius = 60.0;

        ground.apply(&stamp(centre, radius, Brushing::Raise, 10.0, 0.0));

        let middle = ground.at(centre.x, centre.y);
        let halfway = ground.at(centre.x + radius * 0.5, centre.y);
        let beyond = ground.at(centre.x + radius * 1.5, centre.y);

        assert!((middle - 10.0).abs() < 0.5, "middle rose {middle:.2}");
        assert!(halfway > 0.5 && halfway < middle, "halfway {halfway:.2}");
        assert!(beyond.abs() < SCULPT_EPSILON, "beyond the rim {beyond:.2}");
    }

    #[test]
    fn lower_is_the_exact_inverse_of_raise() {
        let mut ground = Sculpt::empty(HALF, SEED);
        ground.apply(&stamp(Vec2::ZERO, 50.0, Brushing::Raise, 7.0, 0.0));
        ground.apply(&stamp(Vec2::ZERO, 50.0, Brushing::Lower, 7.0, 0.0));

        assert!(ground.at(0.0, 0.0).abs() < SCULPT_EPSILON);
        assert_eq!(ground.sculpted_cells(), 0, "no cell left counted as moved");
    }

    #[test]
    fn flatten_converges_on_its_target() {
        let mut ground = Sculpt::empty(HALF, SEED);
        ground.apply(&stamp(Vec2::ZERO, 80.0, Brushing::Raise, 60.0, 0.0));
        for _ in 0..200 {
            ground.apply(&stamp(Vec2::ZERO, 80.0, Brushing::Flatten, 0.1, 25.0));
        }
        let height = ground.at(0.0, 0.0);
        assert!((height - 25.0).abs() < 1.0, "levelled to {height:.2}");
    }

    #[test]
    fn a_road_follows_the_hill_where_flatten_cuts_through_it() {
        // The complaint that changed this: PATH levelled to one height with a
        // flat bed, so on any slope it dug a trench with shoulders. A road takes
        // the bumps out and leaves the hill alone; a cutting removes the hill.
        let slope = |at: Vec2| at.x * 0.15;
        // Bumps a good deal longer than a cell. The offsets are a four-metre
        // grid, so it cannot cancel anything much finer than eight metres across
        // however hard it grades — testing it on ripples that narrow measures
        // the grid's resolution rather than the tool.
        let bumpy = |at: Vec2| slope(at) + (at.x * 0.22).sin() * 1.5;

        let mut road = Sculpt::empty(HALF, SEED);
        let mut cutting = Sculpt::empty(HALF, SEED);
        for _ in 0..60 {
            road.apply(&Stamp {
                centre: Vec2::ZERO,
                radius: 60.0,
                how: Brushing::Path,
                amount: 0.2,
                target: 0.0,
                under: &bumpy,
            });
            cutting.apply(&Stamp {
                centre: Vec2::ZERO,
                radius: 60.0,
                how: Brushing::Flatten,
                amount: 0.2,
                target: 0.0,
                under: &bumpy,
            });
        }

        let finished = |ground: &Sculpt, x: f32| bumpy(Vec2::new(x, 0.0)) + ground.at(x, 0.0);
        // Across the middle of the brush the road should still be climbing at
        // roughly the hill's own grade.
        let fall = (finished(&road, 30.0) - finished(&road, -30.0)) / 60.0;
        assert!(
            (fall - 0.15).abs() < 0.06,
            "a road should keep the hill's grade: {fall:.3} against 0.15"
        );
        // And the levelling tool should have removed it.
        let levelled = (finished(&cutting, 30.0) - finished(&cutting, -30.0)) / 60.0;
        assert!(
            levelled.abs() < 0.05,
            "flatten should take the slope out: {levelled:.3}"
        );

        // The road must also be smoother than the ground it was laid on — that
        // is what makes it passable.
        let roughness = |ground: &Sculpt| {
            (-20..20)
                .map(|i| {
                    let x = i as f32 * 1.5;
                    (finished(ground, x) - (slope(Vec2::new(x, 0.0)))).abs()
                })
                .fold(0.0_f32, f32::max)
        };
        assert!(
            roughness(&road) < 0.7,
            "the bumps should be gone: {:.2} m still standing",
            roughness(&road)
        );
    }

    #[test]
    fn roughen_adds_variation_without_moving_the_average() {
        let mut ground = Sculpt::empty(HALF, SEED);
        ground.apply(&stamp(Vec2::ZERO, 100.0, Brushing::Roughen, 6.0, 0.0));

        let taken: Vec<f32> = (-40..40).map(|i| ground.at(i as f32 * 2.0, 0.0)).collect();
        let mean = taken.iter().sum::<f32>() / taken.len() as f32;
        let spread = taken.iter().map(|v| (v - mean).abs()).fold(0.0, f32::max);

        assert!(spread > 0.5, "roughening should be visible");
        assert!(mean.abs() < 2.0, "it should not lift the ground: {mean:.2}");
    }

    #[test]
    fn smooth_takes_the_edge_off_a_spike() {
        let mut ground = Sculpt::empty(HALF, SEED);
        ground.apply(&stamp(Vec2::ZERO, CELL * 0.6, Brushing::Raise, 100.0, 0.0));
        let before = ground.at(0.0, 0.0);

        for _ in 0..40 {
            ground.apply(&stamp(Vec2::ZERO, 30.0, Brushing::Smooth, 0.5, 0.0));
        }

        let after = ground.at(0.0, 0.0);
        assert!(
            after < before * 0.6,
            "{before:.1} should fall to under 60%, got {after:.1}"
        );
    }

    #[test]
    fn erosion_moves_material_downhill_without_destroying_any() {
        let mut ground = Sculpt::empty(HALF, SEED);
        // A cone far steeper than anything will hold.
        ground.apply(&stamp(Vec2::ZERO, 20.0, Brushing::Raise, 90.0, 0.0));

        let volume = |ground: &Sculpt| {
            let mut total = 0.0;
            for z in -30..30 {
                for x in -30..30 {
                    total += ground.at(x as f32 * CELL, z as f32 * CELL);
                }
            }
            total
        };
        let before = volume(&ground);
        let peak_before = ground.at(0.0, 0.0);
        let foot_before = ground.at(26.0, 0.0);

        for _ in 0..60 {
            ground.apply(&stamp(Vec2::ZERO, 90.0, Brushing::Erode, 1.0, 0.0));
        }

        let peak_after = ground.at(0.0, 0.0);
        let foot_after = ground.at(26.0, 0.0);

        assert!(
            peak_after < peak_before,
            "the peak should shed material: {peak_before:.1} -> {peak_after:.1}"
        );
        assert!(
            foot_after > foot_before,
            "and it should pile at the foot: {foot_before:.1} -> {foot_after:.1}"
        );
        // The whole point of thermal erosion: a hill SETTLES, it does not
        // shrink. Material moves between cells and none of it is invented or
        // lost, so the total is what it was.
        let after = volume(&ground);
        assert!(
            (after - before).abs() < before.abs() * 0.02 + 1.0,
            "material should be conserved: {before:.1} -> {after:.1}"
        );
    }

    #[test]
    fn erosion_leaves_a_slope_it_can_already_hold_alone() {
        let mut ground = Sculpt::empty(HALF, SEED);
        // A gentle rise, well under the angle of repose.
        ground.apply(&stamp(Vec2::ZERO, 300.0, Brushing::Raise, 20.0, 0.0));
        let before: Vec<f32> = (0..40).map(|i| ground.at(i as f32 * CELL, 0.0)).collect();

        for _ in 0..40 {
            ground.apply(&stamp(Vec2::ZERO, 300.0, Brushing::Erode, 1.0, 0.0));
        }

        for (i, was) in before.iter().enumerate() {
            let now = ground.at(i as f32 * CELL, 0.0);
            assert!(
                (now - was).abs() < 0.5,
                "ground already at rest should not move: cell {i}, {was:.2} -> {now:.2}"
            );
        }
    }

    #[test]
    fn every_tool_answers_to_the_strength_control() {
        // Erosion shipped ignoring it: it was lumped in with the tools that
        // converge on a target, which take a fixed blend rate, so the slider
        // moved nothing. Each tool must give a different answer for a weak and a
        // strong setting, whatever "strength" happens to mean to it.
        for how in Brushing::ALL {
            if how.is_two_point() {
                // Laid in one go from two points; it has no per-tick rate.
                continue;
            }
            let weak = how.rate(WEAKEST, 1.0 / 60.0);
            let strong = how.rate(MIDDLING_STRENGTH * 4.0, 1.0 / 60.0);
            assert!(
                strong > weak * 1.5,
                "{} ignores the strength control ({weak} vs {strong})",
                how.name()
            );
        }
    }

    #[test]
    fn erosion_settles_faster_when_told_to_but_no_further() {
        // Strength is a SPEED for this tool, never a distance. A firm setting
        // gets there sooner; neither setting gets anywhere the angle of repose
        // does not allow, because where material comes to rest is the slope's
        // business and not the maker's.
        let mut patient = Sculpt::empty(HALF, SEED);
        let mut hasty = Sculpt::empty(HALF, SEED);
        for ground in [&mut patient, &mut hasty] {
            ground.apply(&stamp(Vec2::ZERO, CELL * 0.6, Brushing::Raise, 140.0, 0.0));
        }

        let slow = Brushing::Erode.rate(WEAKEST, 1.0 / 60.0);
        let fast = Brushing::Erode.rate(MIDDLING_STRENGTH * 4.0, 1.0 / 60.0);
        for _ in 0..4_000 {
            patient.apply(&stamp(Vec2::ZERO, 60.0, Brushing::Erode, slow, 0.0));
            hasty.apply(&stamp(Vec2::ZERO, 60.0, Brushing::Erode, fast, 0.0));
        }

        let held =
            |ground: &Sculpt| (ground.at(0.0, 0.0) - ground.at(CELL * 3.0, 0.0)) / (CELL * 3.0);
        let (slow_slope, fast_slope) = (held(&patient), held(&hasty));

        assert!(
            fast_slope < slow_slope,
            "a firmer setting should settle sooner: {slow_slope:.2} vs {fast_slope:.2}"
        );
        assert!(
            fast_slope <= REPOSE * 1.35,
            "settled steeper than the ground can hold: {fast_slope:.2} vs {REPOSE}"
        );
        assert!(
            fast_slope > 0.0,
            "a hill should settle, not vanish: {fast_slope:.2}"
        );
    }

    #[test]
    fn a_ramp_climbs_evenly_from_one_end_to_the_other() {
        let mut ground = Sculpt::empty(HALF, SEED);
        let from = Vec3::new(-120.0, 10.0, 0.0);
        let to = Vec3::new(120.0, 70.0, 0.0);

        ground.ramp(from, to, 12.0, &flat);

        // Along the middle of the bed the height should be the straight line
        // between the ends - that is what makes it walkable, and what Flatten
        // and Path cannot do.
        for step in 0..=10 {
            let along = step as f32 / 10.0;
            let at = Vec2::new(-120.0, 0.0).lerp(Vec2::new(120.0, 0.0), along);
            let wanted = from.y + (to.y - from.y) * along;
            let got = ground.at(at.x, at.y);
            assert!(
                (got - wanted).abs() < 3.0,
                "at {along:.1} along: wanted {wanted:.1}, got {got:.1}"
            );
        }

        // And it is a road, not a plateau: well off to the side is untouched.
        assert!(
            ground.at(0.0, 90.0).abs() < SCULPT_EPSILON,
            "ground beside the ramp should be left alone"
        );
    }

    #[test]
    fn one_undo_takes_back_a_whole_drag_and_redo_puts_it_back() {
        let mut ground = Sculpt::empty(HALF, SEED);

        ground.begin_stroke();
        // A drag is many ticks, and has to come back in one press.
        for i in 0..25 {
            ground.apply(&stamp(
                Vec2::new(i as f32 * 4.0, 0.0),
                40.0,
                Brushing::Raise,
                2.0,
                0.0,
            ));
        }
        ground.end_stroke();

        let raised = ground.at(40.0, 0.0);
        assert!(raised > 1.0, "the drag should have raised ground");
        assert!(ground.can_undo());

        let changed = ground.undo().expect("undo says what changed");
        assert!(
            ground.at(40.0, 0.0).abs() < SCULPT_EPSILON,
            "the ground should return to exactly where it was"
        );
        assert_eq!(ground.sculpted_cells(), 0);
        let span = across(changed);
        assert!(span.x > 0.0 && span.y > 0.0);

        ground.redo().expect("redo says what changed");
        assert!(
            (ground.at(40.0, 0.0) - raised).abs() < 1.0e-4,
            "redo should restore the drag exactly"
        );
    }

    #[test]
    fn a_fresh_stroke_ends_the_redo_branch() {
        let mut ground = Sculpt::empty(HALF, SEED);

        ground.begin_stroke();
        ground.apply(&stamp(Vec2::ZERO, 40.0, Brushing::Raise, 5.0, 0.0));
        ground.end_stroke();
        ground.undo();
        assert!(ground.can_redo());

        ground.begin_stroke();
        ground.apply(&stamp(Vec2::new(200.0, 0.0), 40.0, Brushing::Raise, 5.0, 0.0));
        ground.end_stroke();

        assert!(!ground.can_redo(), "editing after an undo drops the branch");
    }
}

#[cfg(test)]
mod round_trip {
    use super::*;

    const HALF: Vec2 = Vec2::new(400.0, 300.0);
    const SEED: u32 = 7;

    fn flat(_: Vec2) -> f32 {
        0.0
    }

    #[test]
    fn sculpted_ground_survives_being_written_and_read() {
        let mut kept = Sculpt::empty(HALF, SEED);
        kept.apply(&Stamp {
            centre: Vec2::new(-100.0, 50.0),
            radius: 70.0,
            how: Brushing::Raise,
            amount: 18.0,
            target: 0.0,
            under: &flat,
        });
        assert!(kept.unsaved, "sculpting should mark the layer unsaved");

        let bytes = kept.to_bytes();
        kept.mark_saved();
        assert!(!kept.unsaved, "saying it was written clears the mark");

        let read = Sculpt::read(&bytes, HALF, SEED).expect("it should read back");
        assert_eq!(read.sculpted_cells(), kept.sculpted_cells());
        for probe in [
            Vec2::new(-100.0, 50.0),
            Vec2::new(-70.0, 50.0),
            Vec2::new(200.0, 200.0),
        ] {
            let expected = kept.at(probe.x, probe.y);
            let actual = read.at(probe.x, probe.y);
            assert!(
                (actual - expected).abs() < 1.0e-5,
                "at {probe:?}: wrote {expected:.4}, read {actual:.4}"
            );
        }
    }

    #[test]
    fn ground_from_another_world_is_refused_with_a_reason() {
        // Silence here would smear a maker's afternoon across the map with
        // nothing to say why, so the reason is the point.
        let kept = Sculpt::empty(HALF, SEED);
        let why = Sculpt::read(&kept.to_bytes(), HALF * 2.0, SEED).unwrap_err();
        assert!(why.contains("world"), "unhelpful reason: {why}");

        assert!(Sculpt::read(b"not sculpted ground at all", HALF, SEED).is_err());
        let mut short = kept.to_bytes();
        short.truncate(40);
        assert_eq!(Sculpt::read(&short, HALF, SEED).unwrap_err(), "cut short");
    }
}
