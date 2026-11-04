//! Sprites: flat pictures that turn to face the viewer.

/// How a sprite turns, from `R_DrawSprite`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpriteFacing {
    /// Stands upright, square on to the view direction.
    ViewUpright,
    /// Stands upright, turned towards the viewer.
    FacingUpright,
    /// Lies flat against the view.
    #[default]
    View,
    /// Turned by its entity's angles.
    Oriented,
    /// Lies flat against the view, turned by its entity's roll.
    ViewRolled,
}

/// One picture.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpriteImage {
    /// How far its left edge is from the sprite's origin.
    pub left: i32,
    /// How far its top edge is above the sprite's origin.
    pub top: i32,
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
    /// Its pixels in RGBA, where alpha zero is a hole.
    pub rgba: Vec<u8>,
}

/// A frame a game asks for by number, which may cycle through several pictures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpriteFrame {
    /// The first picture in [`Sprite::images`].
    pub first_image: u32,
    /// How many pictures it has.
    pub image_count: u32,
    /// When each picture ends, in seconds from the start of the cycle.
    pub image_ends: Vec<f32>,
}

impl SpriteFrame {
    /// The picture to show `time` seconds in.
    #[must_use]
    pub fn image_at(&self, time: f32) -> u32 {
        let Some(&cycle) = self.image_ends.last().filter(|cycle| **cycle > 0.0) else {
            return self.first_image;
        };
        let into = time.rem_euclid(cycle);
        let step = self
            .image_ends
            .iter()
            .take_while(|end| **end <= into)
            .count();
        self.first_image + u32::try_from(step).unwrap_or(0).min(self.image_count - 1)
    }
}

/// A sprite.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sprite {
    /// How it turns.
    pub facing: SpriteFacing,
    /// Every picture.
    pub images: Vec<SpriteImage>,
    /// The frames a game asks for by number.
    pub frames: Vec<SpriteFrame>,
}
