//! Stamps: images that turn part of a flat face into relief or carving, with
//! the new surfaces textured (proposal 0086).

use engine_map::MipLevel;

use crate::geometry::{Plane3, Point, Polygon, add_scaled, clip_behind, cross, dot, normalized};
use crate::source::{BrushFace, StampSource};

/// Heights a stamp has each way from its surface.
const LEVELS: f64 = 4.0;

/// A new face from a stamp.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StampFace {
    /// Its corners.
    pub(crate) polygon: Polygon,
    /// How it's textured.
    pub(crate) face: BrushFace,
    /// How far to lift its light samples, to be lit from outside the solid.
    pub(crate) lift: Point,
}

/// A stamp's frame on the surface it's pressed against.
struct Frame {
    /// Along the image's width.
    across: Point,
    /// Up the image.
    up: Point,
    /// Out of the surface.
    out: Point,
}

impl Frame {
    /// The frame of `stamp`, if its normal is one.
    fn of(stamp: &StampSource) -> Option<Self> {
        let out = normalized(stamp.normal)?;
        let reference = if out[2].abs() > 0.9 {
            [0.0, 1.0, 0.0]
        } else {
            [0.0, 0.0, 1.0]
        };
        let side = normalized(cross(reference, out))?;
        let up = cross(out, side);
        let (sin, cos) = stamp.angle.to_radians().sin_cos();
        Some(Self {
            across: add_scaled(side.map(|part| part * cos), up, sin),
            up: add_scaled(up.map(|part| part * cos), side, -sin),
            out,
        })
    }
}

/// The height of each cell, in levels: `columns` by `rows`, from the top.
fn heights(image: &MipLevel, columns: usize, rows: usize) -> Vec<i32> {
    let (width, height) = (image.width as usize, image.height as usize);
    let mut out = Vec::with_capacity(columns * rows);
    for row in 0..rows {
        for column in 0..columns {
            let x = (column * 2 + 1) * width / (columns * 2);
            let y = (row * 2 + 1) * height / (rows * 2);
            let at = (y * width + x) * 4;
            let bright = f64::from(image.rgba.get(at).copied().unwrap_or(128));
            #[expect(clippy::cast_possible_truncation, reason = "a few levels")]
            out.push(((bright / 255.0 - 0.5) * 2.0 * LEVELS).round() as i32);
        }
    }
    out
}

/// A rectangle on the surface, from `(x, y)` to `(end_x, end_y)` in cells,
/// lifted by `height`.
#[derive(Clone, Copy, Debug)]
struct Cells {
    /// Its left column.
    x: usize,
    /// Its top row.
    y: usize,
    /// One past its right column.
    end_x: usize,
    /// One past its bottom row.
    end_y: usize,
}

/// What a stamp puts on one piece of a face.
struct Builder<'a> {
    /// The stamp.
    stamp: &'a StampSource,
    /// Where it sits.
    frame: Frame,
    /// Its corner: the left of its top row, on the surface.
    corner: Point,
    /// A cell's width.
    cell_width: f64,
    /// A cell's height.
    cell_height: f64,
    /// The surface it's pressed against.
    face: &'a BrushFace,
    /// The edges of the piece it's on.
    bounds: Vec<Plane3>,
    /// A level's height.
    step: f64,
}

impl Builder<'_> {
    /// A point at `x` and `y` in cells, at `level`.
    fn point(&self, x: usize, y: usize, level: i32) -> Point {
        #[expect(clippy::cast_precision_loss, reason = "a small grid")]
        let (x, y) = (x as f64 * self.cell_width, y as f64 * self.cell_height);
        let along = add_scaled(self.corner, self.frame.across, x);
        let down = add_scaled(along, self.frame.up, -y);
        add_scaled(down, self.face.plane.normal, f64::from(level) * self.step)
    }

    /// How far to lift what's at `level`, to just past the surface.
    fn lift(&self, level: i32) -> Point {
        let below = (-f64::from(level) * self.step).max(0.0);
        self.face.plane.normal.map(|part| part * (below + 1.0))
    }

    /// The flat top of `cells`, at `level`, kept to the piece it's on.
    fn top(&self, cells: Cells, level: i32, out: &mut Vec<StampFace>) {
        let polygon = vec![
            self.point(cells.x, cells.end_y, level),
            self.point(cells.end_x, cells.end_y, level),
            self.point(cells.end_x, cells.y, level),
            self.point(cells.x, cells.y, level),
        ];
        let Some(polygon) = clip_behind(polygon, &self.bounds, None) else {
            return;
        };
        out.push(StampFace {
            polygon,
            face: self.face.clone(),
            lift: self.lift(level),
        });
    }

    /// The wall from `from` to `to` on the surface, between levels `low` and
    /// `high`, facing `facing`.
    fn wall(
        &self,
        from: (usize, usize),
        to: (usize, usize),
        levels: (i32, i32),
        facing: Point,
        out: &mut Vec<StampFace>,
    ) {
        let (low, high) = levels;
        let polygon = vec![
            self.point(from.0, from.1, low),
            self.point(to.0, to.1, low),
            self.point(to.0, to.1, high),
            self.point(from.0, from.1, high),
        ];
        let Some(polygon) = clip_behind(polygon, &self.bounds, None) else {
            return;
        };
        let polygon = wound_to(polygon, facing);
        let start = polygon[0];
        let material = self
            .stamp
            .inner
            .clone()
            .unwrap_or_else(|| self.face.material.clone());
        out.push(StampFace {
            polygon,
            face: BrushFace {
                plane: Plane3 {
                    normal: facing,
                    distance: dot(facing, start),
                },
                material,
                scale: self.face.scale,
                offset: self.face.offset,
                rotation: 0.0,
            },
            lift: self.lift(low),
        });
    }
}

/// `polygon`, wound to face along `facing`.
fn wound_to(polygon: Polygon, facing: Point) -> Polygon {
    let first = add_scaled(polygon[1], polygon[0], -1.0);
    let second = add_scaled(polygon[2], polygon[0], -1.0);
    if dot(cross(first, second), facing) >= 0.0 {
        polygon
    } else {
        polygon.into_iter().rev().collect()
    }
}

/// The walls of a piece stay inside the column it's stamped on: its edges,
/// standing straight off the surface.
fn side_bounds(polygon: &[Point], normal: Point) -> Vec<Plane3> {
    let mut bounds = Vec::with_capacity(polygon.len());
    let share = 1.0 / polygon.len().max(1) as f64;
    let middle = polygon
        .iter()
        .fold([0.0; 3], |sum, point| add_scaled(sum, *point, share));
    for (index, corner) in polygon.iter().enumerate() {
        let next = polygon[(index + 1) % polygon.len()];
        let Some(mut outward) = normalized(cross(add_scaled(next, *corner, -1.0), normal)) else {
            continue;
        };
        if dot(outward, add_scaled(middle, *corner, -1.0)) > 0.0 {
            outward = outward.map(|part| -part);
        }
        bounds.push(Plane3 {
            normal: outward,
            distance: dot(outward, *corner),
        });
    }
    bounds
}

/// What's left of `polygon` outside a stamp's box, and the faces that take the
/// place of what's inside it, or `None` if the stamp doesn't touch it.
pub(crate) fn stamp_piece(
    stamp: &StampSource,
    image: &MipLevel,
    polygon: &Polygon,
    face: &BrushFace,
) -> Option<(Vec<Polygon>, Vec<StampFace>)> {
    let frame = Frame::of(stamp)?;
    if dot(face.plane.normal, frame.out) < 0.5 {
        return None;
    }
    let [width, height] = stamp.size;
    let wall = |direction: Point, reach: f64| Plane3 {
        normal: direction,
        distance: dot(direction, stamp.origin) + reach,
    };
    let flipped = |direction: Point| direction.map(|part| -part);
    let inside = [
        wall(frame.across, width * 0.5),
        wall(flipped(frame.across), width * 0.5),
        wall(frame.up, height * 0.5),
        wall(flipped(frame.up), height * 0.5),
        wall(frame.out, stamp.depth * 2.0),
        wall(flipped(frame.out), stamp.depth * 2.0),
    ];
    let kept = clip_behind(polygon.clone(), &inside, None)?;
    if kept.len() < 3 {
        return None;
    }
    // The rest of the piece, round the box, a side at a time.
    let mut outside = Vec::new();
    let mut left = polygon.clone();
    for side in &inside[..4] {
        let beyond = Plane3 {
            normal: side.normal.map(|part| -part),
            distance: -side.distance,
        };
        if let Some(piece) = clip_behind(left.clone(), &[beyond], None) {
            outside.push(piece);
        }
        match clip_behind(left, std::slice::from_ref(side), None) {
            Some(rest) => left = rest,
            None => break,
        }
        if left.is_empty() {
            break;
        }
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "sizes are small and positive"
    )]
    let (columns, rows) = (
        ((width / stamp.detail).round() as usize).max(1),
        ((height / stamp.detail).round() as usize).max(1),
    );
    #[expect(clippy::cast_precision_loss, reason = "a small grid")]
    let (cell_width, cell_height) = (width / columns as f64, height / rows as f64);
    let on_face = |point: Point| {
        add_scaled(
            point,
            face.plane.normal,
            face.plane.distance - dot(face.plane.normal, point),
        )
    };
    let corner = on_face(add_scaled(
        add_scaled(stamp.origin, frame.across, -width * 0.5),
        frame.up,
        height * 0.5,
    ));
    let builder = Builder {
        stamp,
        corner,
        cell_width,
        cell_height,
        face,
        bounds: side_bounds(&kept, face.plane.normal),
        step: stamp.depth / LEVELS,
        frame,
    };
    let levels = heights(image, columns, rows);
    let level = |x: usize, y: usize| levels[y * columns + x];
    let mut faces = Vec::new();
    for y in 0..rows {
        let mut x = 0;
        while x < columns {
            let here = level(x, y);
            let mut end = x + 1;
            while end < columns && level(end, y) == here {
                end += 1;
            }
            let cells = Cells {
                x,
                y,
                end_x: end,
                end_y: y + 1,
            };
            builder.top(cells, here, &mut faces);
            x = end;
        }
    }
    let across = builder.frame.across;
    let up = builder.frame.up;
    for y in 0..rows {
        for x in 0..columns {
            let here = level(x, y);
            // Each wall stands on the side of the lower cell, facing it.
            if x + 1 < columns {
                let next = level(x + 1, y);
                if next != here {
                    let facing = if here > next {
                        across
                    } else {
                        across.map(|p| -p)
                    };
                    let levels = (here.min(next), here.max(next));
                    builder.wall((x + 1, y), (x + 1, y + 1), levels, facing, &mut faces);
                }
            }
            if y + 1 < rows {
                let next = level(x, y + 1);
                if next != here {
                    let facing = if here > next { up.map(|p| -p) } else { up };
                    let levels = (here.min(next), here.max(next));
                    builder.wall((x, y + 1), (x + 1, y + 1), levels, facing, &mut faces);
                }
            }
        }
    }
    // Round the rim, down to or up from the surface.
    for y in 0..rows {
        for (x, edge, facing) in [(0, 0, across.map(|p| -p)), (columns - 1, columns, across)] {
            let here = level(x, y);
            if here != 0 {
                let facing = if here > 0 { facing } else { facing.map(|p| -p) };
                let levels = (here.min(0), here.max(0));
                builder.wall((edge, y), (edge, y + 1), levels, facing, &mut faces);
            }
        }
    }
    for x in 0..columns {
        for (y, edge, facing) in [(0, 0, up), (rows - 1, rows, up.map(|p| -p))] {
            let here = level(x, y);
            if here != 0 {
                let facing = if here > 0 { facing } else { facing.map(|p| -p) };
                let levels = (here.min(0), here.max(0));
                builder.wall((x, edge), (x + 1, edge), levels, facing, &mut faces);
            }
        }
    }
    Some((outside, faces))
}
