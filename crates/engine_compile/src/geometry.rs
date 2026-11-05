//! Planes and polygons, in double precision because they're cut many times.

/// A point or direction.
pub type Point = [f64; 3];

/// Points closer than this to a plane count as on it.
pub(crate) const ON_PLANE: f64 = 1e-5;

/// How far a starting polygon reaches. Anything real is cut well inside it.
const WORLD_REACH: f64 = 1_048_576.0;

/// The dot product.
pub(crate) fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The cross product.
pub(crate) fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `a + b * scale`.
pub(crate) fn add_scaled(a: Point, b: Point, scale: f64) -> Point {
    [
        a[0] + b[0] * scale,
        a[1] + b[1] * scale,
        a[2] + b[2] * scale,
    ]
}

/// `a` with length 1, or `None` if it has none.
pub(crate) fn normalized(a: Point) -> Option<Point> {
    let length = dot(a, a).sqrt();
    (length > 1e-12).then(|| [a[0] / length, a[1] / length, a[2] / length])
}

/// A plane: points where `dot(normal, point) == distance`. A brush's planes
/// face outwards.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane3 {
    /// Its normal, of length 1.
    pub normal: Point,
    /// Its distance from the origin along the normal.
    pub distance: f64,
}

impl Plane3 {
    /// How far `point` is in front of the plane.
    #[must_use]
    pub fn side(&self, point: Point) -> f64 {
        dot(self.normal, point) - self.distance
    }

    /// The same plane facing the other way.
    #[must_use]
    pub fn flipped(&self) -> Self {
        Self {
            normal: [-self.normal[0], -self.normal[1], -self.normal[2]],
            distance: -self.distance,
        }
    }
}

/// A polygon's corners.
pub(crate) type Polygon = Vec<Point>;

/// A big square lying on `plane`, wound so its normal is the plane's.
pub(crate) fn plane_polygon(plane: &Plane3) -> Polygon {
    let normal = plane.normal;
    let up = if normal[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let across = normalized(cross(up, normal)).unwrap_or([1.0, 0.0, 0.0]);
    let along = cross(normal, across);
    let centre = add_scaled([0.0; 3], normal, plane.distance);
    let corner = |a: f64, b: f64| {
        add_scaled(
            add_scaled(centre, across, a * WORLD_REACH),
            along,
            b * WORLD_REACH,
        )
    };
    vec![
        corner(-1.0, -1.0),
        corner(1.0, -1.0),
        corner(1.0, 1.0),
        corner(-1.0, 1.0),
    ]
}

/// Splits `polygon` by `plane`, into what's in front and what's behind. A
/// polygon lying on the plane comes back as `on`.
pub(crate) enum Split {
    /// All of it is in front.
    Front,
    /// All of it is behind.
    Back,
    /// It lies on the plane.
    On,
    /// It crosses, into these two.
    Both(Polygon, Polygon),
}

/// Splits `polygon` by `plane`.
pub(crate) fn split(polygon: &[Point], plane: &Plane3) -> Split {
    let sides: Vec<f64> = polygon.iter().map(|point| plane.side(*point)).collect();
    let front = sides.iter().any(|side| *side > ON_PLANE);
    let back = sides.iter().any(|side| *side < -ON_PLANE);
    match (front, back) {
        (false, false) => return Split::On,
        (true, false) => return Split::Front,
        (false, true) => return Split::Back,
        (true, true) => {}
    }
    let (mut ahead, mut behind) = (Vec::new(), Vec::new());
    for index in 0..polygon.len() {
        let next = (index + 1) % polygon.len();
        let (point, side) = (polygon[index], sides[index]);
        let (other, other_side) = (polygon[next], sides[next]);
        if side >= -ON_PLANE {
            ahead.push(point);
        }
        if side <= ON_PLANE {
            behind.push(point);
        }
        let crosses = (side > ON_PLANE && other_side < -ON_PLANE)
            || (side < -ON_PLANE && other_side > ON_PLANE);
        if crosses {
            let share = side / (side - other_side);
            let middle = [
                point[0] + (other[0] - point[0]) * share,
                point[1] + (other[1] - point[1]) * share,
                point[2] + (other[2] - point[2]) * share,
            ];
            ahead.push(middle);
            behind.push(middle);
        }
    }
    Split::Both(ahead, behind)
}

/// What's left of `polygon` behind every one of `planes`, skipping `skip`.
pub(crate) fn clip_behind(
    mut polygon: Polygon,
    planes: &[Plane3],
    skip: Option<usize>,
) -> Option<Polygon> {
    for (index, plane) in planes.iter().enumerate() {
        if Some(index) == skip {
            continue;
        }
        match split(&polygon, plane) {
            Split::Front => return None,
            Split::Back | Split::On => {}
            Split::Both(_, behind) => polygon = behind,
        }
        if polygon.len() < 3 {
            return None;
        }
    }
    Some(polygon)
}

/// Every corner of the convex solid behind all `planes`.
pub(crate) fn solid_corners(planes: &[Plane3]) -> Vec<Point> {
    let mut corners = Vec::new();
    for (index, plane) in planes.iter().enumerate() {
        if let Some(polygon) = clip_behind(plane_polygon(plane), planes, Some(index)) {
            corners.extend(polygon);
        }
    }
    corners
}
