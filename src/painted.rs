//! A grid a maker painted over the world.
//!
//! Signed bias in cells, where **zero leaves the ground's own answer alone** —
//! positive forces the thing on, negative forces it off, and only an undo gets
//! back to no opinion at all. That one property is what lets an authored layer
//! sit on top of generated terrain without freezing it: re-roll the noise and a
//! painted wood is still a wood, because what was written was a *decision* and
//! not a result.
//!
//! # Two layers, one grid
//!
//! The woods were the first. Surface came second and wanted exactly the same
//! thing — cells, a falloff, an undo stack, a file that refuses to be read into
//! the wrong world — differing only in how fine the cells are and what the file
//! is called. So the grid is written once and the [`Kind`] says which layer it
//! is. A second copy of this, drifting, is the failure mode the whole crate
//! exists to prevent.

use std::collections::HashMap;

use glam::Vec2;

use crate::history::History;
use crate::smoothstep;
use crate::Patch;

/// Below this, a cell is untouched and the ground's answer stands.
pub const EPSILON: f32 = 0.01;

/// How many strokes can be taken back. The same depth the ground keeps, because
/// a maker pressing the same key expects the same reach.
const UNDO_DEPTH: usize = 64;

/// Which painted layer this is.
///
/// The two differ in exactly two ways and it is worth being able to see both at
/// once: how fine the cells are, and what the file is called.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Where trees stand. Coarse — a wood is not placed to the metre, and a
    /// world of it at this size is a few hundred kilobytes.
    Woods,
    /// What the ground is made of: dirt over the grass, for roads and yards and
    /// the worn ground round a door.
    ///
    /// Four times finer than the woods, because a road is about six metres wide
    /// and a sixteen-metre cell cannot draw one — it would come out a field.
    Surface,
    /// Which COUNTRY the ground belongs to: the green world, desert, snow.
    ///
    /// # Why this layer exists at all
    ///
    /// The regions were placed in code — a band here, an oval there — and moving
    /// one meant somebody reading a marker's position off a screenshot and
    /// guessing which number it implied. That went wrong five times running,
    /// because the person who can SEE where the desert should be and the person
    /// who can edit the number were not the same person.
    ///
    /// So the map gets painted instead. This layer overrules what the code would
    /// have said, and where it is empty the code still answers — a fresh world
    /// still has a world in it.
    ///
    /// As coarse as the woods. A country is kilometres across; nobody places one
    /// to the metre.
    Country,
}

impl Kind {
    /// Names the file, so a stale or unrelated one is refused rather than read
    /// as the wrong layer entirely.
    fn magic(self) -> &'static [u8; 8] {
        match self {
            Kind::Woods => b"RNGRFST1",
            Kind::Surface => b"RNGRSRF1",
            Kind::Country => b"RNGRCTY1",
        }
    }

    /// Metres per cell. `const`, so a layer's size can be named where a
    /// constant is wanted rather than copied into one.
    pub const fn cell(self) -> f32 {
        match self {
            Kind::Woods => 16.0,
            Kind::Surface => 4.0,
            Kind::Country => 16.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Woods => "woods",
            Kind::Surface => "surface",
            Kind::Country => "country",
        }
    }
}

/// What a maker painted, for one layer.
pub struct Painted {
    kind: Kind,
    wide: usize,
    deep: usize,
    half: Vec2,
    bias: Vec<f32>,
    painted: usize,
    history: History,
    /// Whether anything has been painted since this was last written.
    pub unsaved: bool,
}

impl std::fmt::Debug for Painted {
    // Its shape and how much is painted, never the whole grid: a world of it is
    // millions of cells and dumping them helps nobody.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Painted {{ {} {}x{} cells over {:.0}x{:.0} m, {} painted }}",
            self.kind.name(),
            self.wide,
            self.deep,
            self.half.x * 2.0,
            self.half.y * 2.0,
            self.painted
        )
    }
}

impl Painted {
    /// An empty layer: the world exactly as it would be with nobody's opinion.
    pub fn empty(kind: Kind, half: Vec2) -> Self {
        let cell = kind.cell();
        let wide = (half.x * 2.0 / cell).ceil() as usize + 1;
        let deep = (half.y * 2.0 / cell).ceil() as usize + 1;
        Self {
            kind,
            wide,
            deep,
            half,
            bias: vec![0.0; wide * deep],
            painted: 0,
            history: History::new(UNDO_DEPTH),
            unsaved: false,
        }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Reads a painted layer from bytes.
    ///
    /// Takes BYTES, not a path, and returns the reason on failure rather than
    /// logging it. Where the file lives and how a problem is reported are each
    /// program's own business — this crate is linked by two of them and has no
    /// business deciding either.
    pub fn read(bytes: &[u8], kind: Kind, half: Vec2) -> Result<Self, String> {
        let empty = Self::empty(kind, half);
        let header = 8 + 4 * 4;
        if bytes.len() < header || &bytes[..8] != kind.magic() {
            return Err(format!("not a painted {}", kind.name()));
        }

        let word = |at: usize| {
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
        };
        let real =
            |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
        let (wide, deep) = (word(8), word(12));
        let saved_half = Vec2::new(real(16), real(20));

        // Refused rather than stretched. Paint landing in the wrong places is
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
        let painted = bias.iter().filter(|v| v.abs() > EPSILON).count();
        Ok(Self {
            bias,
            painted,
            ..empty
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + 16 + self.bias.len() * 4);
        out.extend_from_slice(self.kind.magic());
        out.extend_from_slice(&(self.wide as u32).to_le_bytes());
        out.extend_from_slice(&(self.deep as u32).to_le_bytes());
        out.extend_from_slice(&self.half.x.to_le_bytes());
        out.extend_from_slice(&self.half.y.to_le_bytes());
        for value in &self.bias {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }

    /// Says the bytes reached a file. Separate from [`Self::to_bytes`] because
    /// only the caller knows whether the write actually landed.
    pub fn mark_saved(&mut self) {
        self.unsaved = false;
    }

    /// Paints, positive to force on and negative to force off. Returns the
    /// ground it changed.
    ///
    /// It lives here because paint laid at the bench and paint laid in the game
    /// must be the same paint, and two implementations of the falloff would not
    /// be.
    pub fn paint(&mut self, centre: Vec2, radius: f32, amount: f32) -> Patch {
        self.paint_with(centre, radius, amount, |away, radius| {
            smoothstep(radius, 0.0, away)
        })
    }

    /// Paints with a falloff of the caller's choosing.
    ///
    /// A road wants a flat bed and quick shoulders where a wood wants a soft
    /// dish, and that is a property of the TOOL rather than of the layer — so
    /// the shape comes in rather than being decided here.
    pub fn paint_with(
        &mut self,
        centre: Vec2,
        radius: f32,
        amount: f32,
        falloff: impl Fn(f32, f32) -> f32,
    ) -> Patch {
        let cell = self.kind.cell();
        let to_cell = |v: f32, half: f32, count: usize| {
            (((v + half) / cell).floor() as isize).clamp(0, count as isize - 1) as usize
        };
        let x0 = to_cell(centre.x - radius, self.half.x, self.wide);
        let x1 = to_cell(centre.x + radius + cell, self.half.x, self.wide);
        let z0 = to_cell(centre.y - radius, self.half.y, self.deep);
        let z1 = to_cell(centre.y + radius + cell, self.half.y, self.deep);

        for z in z0..=z1 {
            for x in x0..=x1 {
                let at = self.cell_at(x, z);
                let away = at.distance(centre);
                if away > radius {
                    continue;
                }
                let shape = falloff(away, radius);
                if shape <= 0.0 {
                    continue;
                }
                let index = z * self.wide + x;
                let now = (self.bias[index] + amount * shape).clamp(-1.0, 1.0);
                self.history.record(index, self.bias[index]);
                self.write(index, now);
            }
        }

        self.unsaved = true;
        (
            centre - Vec2::splat(radius + cell),
            centre + Vec2::splat(radius + cell),
        )
    }

    /// Writes one exact value across the brush, rather than adding to what is
    /// there.
    ///
    /// # Why a layer needs this as well as `paint`
    ///
    /// A bias is a QUANTITY: more trees, fewer, a bit more. Adding to it and
    /// clamping is exactly right, and `paint` does that.
    ///
    /// A country is a CHOICE. There is no such thing as sixty per cent desert in
    /// a cell — you cannot accumulate your way from grass to snow, and the clamp
    /// to plus or minus one that keeps a bias sane would refuse to store a third
    /// option at all. So the value is stamped: whatever is under the brush becomes
    /// this, and nothing is mixed.
    pub fn stamp(&mut self, centre: Vec2, radius: f32, value: f32) -> Patch {
        let cell = self.kind.cell();
        let to_cell = |v: f32, half: f32, count: usize| {
            (((v + half) / cell).floor() as isize).clamp(0, count as isize - 1) as usize
        };
        let x0 = to_cell(centre.x - radius, self.half.x, self.wide);
        let x1 = to_cell(centre.x + radius + cell, self.half.x, self.wide);
        let z0 = to_cell(centre.y - radius, self.half.y, self.deep);
        let z1 = to_cell(centre.y + radius + cell, self.half.y, self.deep);

        for z in z0..=z1 {
            for x in x0..=x1 {
                let index = z * self.wide + x;
                if self.cell_at(x, z).distance(centre) > radius {
                    continue;
                }
                if self.bias[index] == value {
                    continue;
                }
                self.history.record(index, self.bias[index]);
                self.write(index, value);
            }
        }

        self.unsaved = true;
        (
            centre - Vec2::splat(radius + cell),
            centre + Vec2::splat(radius + cell),
        )
    }

    /// What was stamped here, and how much of the neighbourhood agrees.
    ///
    /// # A choice cannot be read between cells
    ///
    /// [`Self::at`] blends the four cells around a point, which is right for a
    /// quantity and nonsense for a choice: halfway between grass and desert is not
    /// a number, and reading one would put snow wherever a two met a four.
    ///
    /// So the four cells VOTE. The one with the most bilinear weight behind it
    /// wins the point outright, and the share of the weight it carried comes back
    /// as well — which is what gives a painted edge a soft side without ever
    /// inventing a country nobody painted. A point in the middle of a painted area
    /// carries the whole vote; one on the boundary carries half.
    ///
    /// `None` where nothing has been painted, so a caller can fall through to
    /// whatever the world would have said for itself.
    pub fn choice(&self, x: f32, z: f32) -> Option<(f32, f32)> {
        let cell = self.kind.cell();
        let fx = (x + self.half.x) / cell;
        let fz = (z + self.half.y) / cell;
        if fx < 0.0 || fz < 0.0 || fx > (self.wide - 1) as f32 || fz > (self.deep - 1) as f32 {
            return None;
        }
        let x0 = fx.floor() as usize;
        let z0 = fz.floor() as usize;
        let x1 = (x0 + 1).min(self.wide - 1);
        let z1 = (z0 + 1).min(self.deep - 1);
        let tx = fx - x0 as f32;
        let tz = fz - z0 as f32;

        let corners = [
            (x0, z0, (1.0 - tx) * (1.0 - tz)),
            (x1, z0, tx * (1.0 - tz)),
            (x0, z1, (1.0 - tx) * tz),
            (x1, z1, tx * tz),
        ];

        let mut best = (0.0_f32, 0.0_f32);
        for (cx, cz, _) in corners {
            let value = self.bias[cz * self.wide + cx];
            if value == 0.0 {
                continue;
            }
            // Every corner holding this same value, added up.
            let weight: f32 = corners
                .iter()
                .filter(|(ox, oz, _)| self.bias[oz * self.wide + ox] == value)
                .map(|(_, _, w)| w)
                .sum();
            if weight > best.1 {
                best = (value, weight);
            }
        }

        (best.1 > 0.0).then_some(best)
    }

    /// Fades the paint back toward no opinion at all.
    ///
    /// Not the same as painting negative, and the difference is the whole point
    /// of a signed bias: painting negative WRITES a decision to force the thing
    /// off, and holds it off against whatever the ground would have said. Zero
    /// is the ground answering for itself, and only this gets back to it.
    pub fn fade(&mut self, centre: Vec2, radius: f32, amount: f32) -> Patch {
        let cell = self.kind.cell();
        let to_cell = |v: f32, half: f32, count: usize| {
            (((v + half) / cell).floor() as isize).clamp(0, count as isize - 1) as usize
        };
        let x0 = to_cell(centre.x - radius, self.half.x, self.wide);
        let x1 = to_cell(centre.x + radius + cell, self.half.x, self.wide);
        let z0 = to_cell(centre.y - radius, self.half.y, self.deep);
        let z1 = to_cell(centre.y + radius + cell, self.half.y, self.deep);

        for z in z0..=z1 {
            for x in x0..=x1 {
                let at = self.cell_at(x, z);
                let away = at.distance(centre);
                if away > radius {
                    continue;
                }
                let t = (amount * smoothstep(radius, 0.0, away)).clamp(0.0, 1.0);
                if t <= 0.0 {
                    continue;
                }
                let index = z * self.wide + x;
                self.history.record(index, self.bias[index]);
                let faded = self.bias[index] * (1.0 - t);
                // Snapped the last of the way, or a cell approaches zero for
                // ever and counts as painted while holding a millionth.
                let faded = if faded.abs() < EPSILON { 0.0 } else { faded };
                self.write(index, faded);
            }
        }

        self.unsaved = true;
        (
            centre - Vec2::splat(radius + cell),
            centre + Vec2::splat(radius + cell),
        )
    }

    // ------------------------------------------------------------ taking back

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

    fn put_back(&mut self, values: &HashMap<usize, f32>) -> HashMap<usize, f32> {
        let mut inverse = HashMap::with_capacity(values.len());
        for (&index, &value) in values {
            inverse.insert(index, self.bias[index]);
            self.write(index, value);
        }
        self.unsaved = true;
        inverse
    }

    /// The ground a set of cells covers, padded by one because reading is
    /// bilinear and reaches a cell past whatever was written.
    fn ground_of(&self, values: &HashMap<usize, f32>) -> Patch {
        let cell = self.kind.cell();
        let mut low = Vec2::splat(f32::MAX);
        let mut high = Vec2::splat(f32::MIN);
        for &index in values.keys() {
            let at = self.cell_at(index % self.wide, index / self.wide);
            low = low.min(at);
            high = high.max(at);
        }
        (low - cell, high + cell)
    }

    // -------------------------------------------------------------- questions

    pub fn painted_cells(&self) -> usize {
        self.painted
    }

    /// The bias at a world position, read between cells.
    pub fn at(&self, x: f32, z: f32) -> f32 {
        let cell = self.kind.cell();
        let fx = (x + self.half.x) / cell;
        let fz = (z + self.half.y) / cell;
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

    fn cell_at(&self, x: usize, z: usize) -> Vec2 {
        let cell = self.kind.cell();
        Vec2::new(
            x as f32 * cell - self.half.x,
            z as f32 * cell - self.half.y,
        )
    }

    /// Writes one cell, keeping the painted count in step.
    fn write(&mut self, index: usize, value: f32) {
        let was = self.bias[index].abs() > EPSILON;
        let is = value.abs() > EPSILON;
        match (was, is) {
            (false, true) => self.painted += 1,
            (true, false) => self.painted -= 1,
            _ => {}
        }
        self.bias[index] = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stamped_choice_reads_back_as_itself() {
        // The fault this exists to avoid: reading a CHOICE the way a quantity is
        // read. Blending the four cells around a point turns a two beside a four
        // into a three — grass beside snow reading as desert, a country nobody
        // painted appearing along every boundary between two that somebody did.
        let mut layer = Painted::empty(Kind::Country, Vec2::splat(400.0));
        layer.stamp(Vec2::new(-100.0, 0.0), 60.0, 2.0);
        layer.stamp(Vec2::new(100.0, 0.0), 60.0, 4.0);

        assert_eq!(layer.choice(-100.0, 0.0).map(|(v, _)| v), Some(2.0));
        assert_eq!(layer.choice(100.0, 0.0).map(|(v, _)| v), Some(4.0));

        // Nothing but those two, anywhere along the line between them — and in
        // particular never a three.
        for step in 0..=200 {
            let x = -140.0 + step as f32 * 1.4;
            if let Some((value, share)) = layer.choice(x, 0.0) {
                assert!(
                    value == 2.0 || value == 4.0,
                    "reading {value} at x={x:.0}, which nobody painted"
                );
                assert!(share > 0.0 && share <= 1.0001, "share {share} at x={x:.0}");
            }
        }

        // Unpainted ground says nothing at all, so a caller can fall through to
        // whatever the world would have decided for itself.
        assert_eq!(layer.choice(0.0, 300.0), None);
    }

    #[test]
    fn a_stamped_edge_has_a_soft_side() {
        // The share is what gives a painted country somewhere to fade, without
        // ever inventing one. Deep inside, every corner agrees; at the rim, some
        // of them have not been painted.
        let mut layer = Painted::empty(Kind::Country, Vec2::splat(400.0));
        layer.stamp(Vec2::ZERO, 100.0, 3.0);

        let middle = layer.choice(0.0, 0.0).expect("painted at the middle").1;
        assert!(middle > 0.99, "the middle only carries {middle:.2} of the vote");

        // Somewhere out at the rim the vote is split.
        let split = (0..40)
            .map(|step| 80.0 + step as f32 * 1.0)
            .filter_map(|x| layer.choice(x, 0.0))
            .any(|(_, share)| share < 0.9);
        assert!(split, "the edge of a stamp carries a full vote everywhere");
    }

    const HALF: Vec2 = Vec2::new(800.0, 600.0);

    #[test]
    fn painting_survives_being_written_and_read() {
        for kind in [Kind::Woods, Kind::Surface] {
            let mut painted = Painted::empty(kind, HALF);
            painted.paint(Vec2::new(100.0, -50.0), 80.0, 1.0);
            assert!(painted.at(100.0, -50.0) > 0.9, "{kind:?}: middle painted");

            let read = Painted::read(&painted.to_bytes(), kind, HALF).expect("should read back");
            assert_eq!(read.painted_cells(), painted.painted_cells());
            assert!((read.at(100.0, -50.0) - painted.at(100.0, -50.0)).abs() < 1.0e-5);
        }
    }

    #[test]
    fn one_layer_will_not_be_read_as_the_other() {
        // The whole point of a magic. Woods read as surface would pave every
        // forest in the world and say nothing.
        let woods = Painted::empty(Kind::Woods, HALF);
        let why = Painted::read(&woods.to_bytes(), Kind::Surface, HALF).unwrap_err();
        assert!(why.contains("surface"), "unhelpful reason: {why}");
    }

    #[test]
    fn a_layer_from_another_world_is_refused_with_a_reason() {
        let painted = Painted::empty(Kind::Woods, HALF);
        let why = Painted::read(&painted.to_bytes(), Kind::Woods, HALF * 2.0).unwrap_err();
        assert!(why.contains("world"), "unhelpful reason: {why}");

        let mut short = painted.to_bytes();
        short.truncate(40);
        assert_eq!(
            Painted::read(&short, Kind::Woods, HALF).unwrap_err(),
            "truncated"
        );
    }

    #[test]
    fn the_surface_is_fine_enough_to_draw_a_road() {
        // Six metres is a cart road. On the woods' sixteen-metre cells that is
        // one cell wide at best and comes out a field, which is why the layers
        // do not share a size.
        let mut painted = Painted::empty(Kind::Surface, HALF);
        painted.paint(Vec2::ZERO, 3.0, 1.0);
        assert!(painted.at(0.0, 0.0) > 0.5, "the road should be laid");
        assert!(
            painted.at(12.0, 0.0).abs() < EPSILON,
            "and it should not spread four times its width"
        );
    }

    #[test]
    fn a_flat_bed_is_not_a_soft_dish() {
        // A road is flat across its width with quick shoulders; a wood fades
        // from its middle. Same grid, different tool.
        let mut road = Painted::empty(Kind::Surface, HALF);
        let mut dish = Painted::empty(Kind::Surface, HALF);
        road.paint_with(Vec2::ZERO, 40.0, 1.0, |away, radius| {
            smoothstep(radius, radius * 0.7, away)
        });
        dish.paint(Vec2::ZERO, 40.0, 1.0);

        let probe = 20.0;
        assert!(
            road.at(probe, 0.0) > dish.at(probe, 0.0) + 0.2,
            "halfway out: road {:.2}, dish {:.2}",
            road.at(probe, 0.0),
            dish.at(probe, 0.0)
        );
    }

    #[test]
    fn one_undo_takes_back_a_whole_drag() {
        let mut painted = Painted::empty(Kind::Woods, HALF);
        painted.begin_stroke();
        for i in 0..20 {
            painted.paint(Vec2::new(i as f32 * 8.0, 0.0), 40.0, 0.2);
        }
        painted.end_stroke();
        assert!(painted.can_undo());

        painted.undo().expect("undo says what changed");
        assert_eq!(painted.painted_cells(), 0, "back to no opinion at all");
    }

    #[test]
    fn fading_gets_back_to_zero_where_clearing_cannot() {
        // Clearing WRITES a decision to force the thing off and holds it there.
        // Zero is the ground answering for itself, and only fading reaches it.
        let mut cleared = Painted::empty(Kind::Surface, HALF);
        cleared.paint(Vec2::ZERO, 40.0, 1.0);
        cleared.paint(Vec2::ZERO, 40.0, -2.0);
        assert!(
            cleared.at(0.0, 0.0) < -EPSILON,
            "clearing overshoots into forcing it off: {:.2}",
            cleared.at(0.0, 0.0)
        );

        let mut faded = Painted::empty(Kind::Surface, HALF);
        faded.paint(Vec2::ZERO, 40.0, 1.0);
        for _ in 0..40 {
            faded.fade(Vec2::ZERO, 40.0, 0.3);
        }
        assert_eq!(faded.at(0.0, 0.0), 0.0, "fading should reach exactly zero");
        // The rim of a falloff brush does least, so a single stamp leaves a
        // faint edge. What must be true is that the middle reaches nothing at
        // all — which painting negative can never do.
        assert!(
            faded.at(10.0, 0.0).abs() < 0.2,
            "well inside the brush should be clean: {:.3}",
            faded.at(10.0, 0.0)
        );
    }

    #[test]
    fn painting_says_it_needs_saving() {
        let mut painted = Painted::empty(Kind::Surface, HALF);
        assert!(!painted.unsaved, "a fresh layer owes nothing");
        painted.paint(Vec2::ZERO, 40.0, 1.0);
        assert!(painted.unsaved);
        painted.mark_saved();
        assert!(!painted.unsaved);
    }
}
