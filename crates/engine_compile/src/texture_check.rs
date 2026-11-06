//! Checks a picture against the texture standard (proposal 0104).

/// The smallest side a picture can have.
const SMALLEST: u32 = 64;

/// The largest side a picture can have.
const LARGEST: u32 = 2048;

/// How many rows either side of an edge are looked at for a border.
const BORDER_ROWS: usize = 2;

/// How much darker or brighter an edge has to be than what's by it to be a border,
/// at least.
const BORDER_DEPTH: f32 = 2.0;

/// How many times more than a usual row an edge has to stand out to be a border.
const BORDER_TIMES: f32 = 4.0;

/// How many times a typical step between rows a step over a repeat can be.
const MOST_STEP: f32 = 3.0;

/// How a picture repeats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    /// In both directions.
    Both,
    /// From side to side.
    Across,
    /// Not at all: it's shown whole.
    None,
}

impl Repeat {
    /// The value of a material's `repeat` field.
    #[must_use]
    pub fn from_text(text: &str) -> Option<Self> {
        match text {
            "both" => Some(Self::Both),
            "across" => Some(Self::Across),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

/// A picture's brightness at each pixel, a row at a time.
struct Brightness {
    /// Pixels across.
    width: usize,
    /// Pixels down.
    height: usize,
    /// The values.
    values: Vec<f32>,
}

impl Brightness {
    /// The brightness of `rgba`.
    fn of(width: u32, height: u32, rgba: &[u8]) -> Self {
        let values = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|[r, g, b, _]| (f32::from(*r) + f32::from(*g) + f32::from(*b)) / 3.0)
            .collect();
        Self {
            width: width as usize,
            height: height as usize,
            values,
        }
    }

    /// The same picture on its side, so columns can be read as rows.
    fn turned(&self) -> Self {
        let mut values = Vec::with_capacity(self.values.len());
        for x in 0..self.width {
            for y in 0..self.height {
                values.push(self.values[y * self.width + x]);
            }
        }
        Self {
            width: self.height,
            height: self.width,
            values,
        }
    }

    /// Row `y`.
    fn row(&self, y: usize) -> &[f32] {
        &self.values[y * self.width..(y + 1) * self.width]
    }

    /// How bright row `y` is on average.
    #[expect(clippy::cast_precision_loss, reason = "picture sizes are small")]
    fn mean(&self, y: usize) -> f32 {
        self.row(y).iter().sum::<f32>() / self.width as f32
    }

    /// How different two rows are on average.
    #[expect(clippy::cast_precision_loss, reason = "picture sizes are small")]
    fn step(&self, a: usize, b: usize) -> f32 {
        let pairs = self.row(a).iter().zip(self.row(b));
        pairs.map(|(a, b)| (a - b).abs()).sum::<f32>() / self.width as f32
    }

    /// How far row `y` stands out from those either side of it, wrapping.
    fn stands_out(&self, y: usize) -> f32 {
        let height = self.height;
        let before = self.mean((y + height - BORDER_ROWS) % height);
        let after = self.mean((y + BORDER_ROWS) % height);
        (self.mean(y) - (before + after) / 2.0).abs()
    }

    /// Whether the rows at the top or bottom stand out more than any row does.
    #[expect(clippy::cast_precision_loss, reason = "picture sizes are small")]
    fn has_border(&self) -> bool {
        let usual = (0..self.height).map(|y| self.stands_out(y)).sum::<f32>() / self.height as f32;
        let edge = self.stands_out(0).max(self.stands_out(self.height - 1));
        edge > BORDER_DEPTH.max(usual * BORDER_TIMES)
    }

    /// How the step over the repeat compares with a typical one inside.
    #[expect(clippy::cast_precision_loss, reason = "picture sizes are small")]
    fn repeat_step(&self) -> f32 {
        let steps: f32 = (1..self.height).map(|y| self.step(y - 1, y)).sum();
        let typical = steps / (self.height - 1) as f32;
        self.step(self.height - 1, 0) / typical.max(0.5)
    }
}

/// What's wrong with a picture, in plain words.
#[must_use]
pub fn check_picture(width: u32, height: u32, rgba: &[u8], repeat: Repeat) -> Vec<String> {
    let mut problems = Vec::new();
    for (name, side) in [("width", width), ("height", height)] {
        if !side.is_power_of_two() {
            problems.push(format!("its {name} of {side} isn't a power of two"));
        }
        if !(SMALLEST..=LARGEST).contains(&side) {
            problems.push(format!(
                "its {name} of {side} isn't {SMALLEST} to {LARGEST}"
            ));
        }
    }
    if width < 8 || height < 8 {
        return problems;
    }
    let down = Brightness::of(width, height, rgba);
    let across = down.turned();
    if repeat != Repeat::None {
        if across.has_border() {
            problems.push("it has a border down its sides".to_owned());
        }
        let step = across.repeat_step();
        if step > MOST_STEP {
            problems.push(format!(
                "its sides don't meet: a step {step:.1} times normal"
            ));
        }
    }
    if repeat == Repeat::Both {
        if down.has_border() {
            problems.push("it has a border along its top and bottom".to_owned());
        }
        let step = down.repeat_step();
        if step > MOST_STEP {
            problems.push(format!(
                "its top and bottom don't meet: a step {step:.1} times normal"
            ));
        }
    }
    problems
}
