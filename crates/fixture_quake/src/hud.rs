//! What Quake shows over the view: the status bar, the end of level screens,
//! text in the middle of the screen and the colour of the view. From `sbar.c`,
//! `screen.c` and `view.c`, for one player (proposal 0040).

use engine_core::canvas::{CHARACTER_SIZE, Canvas};
use engine_map::Contents;

use crate::texture::Palette;

/// `STAT_HEALTH`.
const STAT_HEALTH: usize = 0;
/// `STAT_AMMO`.
const STAT_AMMO: usize = 3;
/// `STAT_ARMOR`.
const STAT_ARMOR: usize = 4;
/// `STAT_SHELLS`, the first of four ammo counts.
const STAT_SHELLS: usize = 6;
/// `STAT_ACTIVEWEAPON`.
const STAT_ACTIVE_WEAPON: usize = 10;
/// `STAT_TOTALSECRETS`.
const STAT_TOTAL_SECRETS: usize = 11;
/// `STAT_TOTALMONSTERS`.
const STAT_TOTAL_MONSTERS: usize = 12;
/// `STAT_SECRETS`.
const STAT_SECRETS: usize = 13;
/// `STAT_MONSTERS`.
const STAT_MONSTERS: usize = 14;
/// The items, which the original sends apart from its stats.
const STAT_ITEMS: usize = 15;
/// Whether this is deathmatch.
const STAT_DEATHMATCH: usize = 16;
/// How many stats there are.
const STAT_COUNT: usize = 17;

/// `IT_SHOTGUN`, the first of seven weapons.
const IT_SHOTGUN: u32 = 1;
/// `IT_SHELLS`.
const IT_SHELLS: u32 = 256;
/// `IT_NAILS`.
const IT_NAILS: u32 = 512;
/// `IT_ROCKETS`.
const IT_ROCKETS: u32 = 1024;
/// `IT_CELLS`.
const IT_CELLS: u32 = 2048;
/// `IT_ARMOR1`.
const IT_ARMOR1: u32 = 8192;
/// `IT_ARMOR2`.
const IT_ARMOR2: u32 = 16384;
/// `IT_ARMOR3`.
const IT_ARMOR3: u32 = 32768;
/// `IT_INVISIBILITY`.
const IT_INVISIBILITY: u32 = 1 << 19;
/// `IT_INVULNERABILITY`.
const IT_INVULNERABILITY: u32 = 1 << 20;
/// `IT_SUIT`.
const IT_SUIT: u32 = 1 << 21;
/// `IT_QUAD`.
const IT_QUAD: u32 = 1 << 22;

/// The bit of the first item on the bar.
const FIRST_ITEM_BIT: u32 = 17;
/// The bit of the first sigil.
const FIRST_SIGIL_BIT: u32 = 28;

/// How wide the screen has to be for scores beside the bar.
const MINI_RANKING_WIDTH: f32 = 512.0;
/// How many fit beside the bar and the inventory.
const MINI_RANKING_LINES: usize = 6;
/// `SBAR_HEIGHT`.
const BAR_HEIGHT: f32 = 24.0;
/// The bar's width, which it's centred by.
const BAR_WIDTH: f32 = 320.0;
/// How far apart big numbers are.
const DIGIT_WIDTH: f32 = 24.0;

/// `scr_centertime`, in seconds.
const CENTER_SECONDS: f64 = 2.0;
/// `scr_printspeed`, in characters a second.
const PRINT_SPEED: f64 = 8.0;
/// Where a long message starts.
const LONG_MESSAGE_TOP: f32 = 48.0;
/// The longest line in the middle of the screen.
const MAX_CENTER_LINE: usize = 40;

/// How long the face shows pain, in seconds.
const FACE_PAIN_SECONDS: f64 = 0.2;

/// The weapon pictures, in the order of their bits.
const WEAPON_PICTURES: [&str; 7] = [
    "shotgun", "sshotgun", "nailgun", "snailgun", "rlaunch", "srlaunch", "lightng",
];

/// The item pictures, in the order of their bits.
const ITEM_PICTURES: [&str; 6] = [
    "sb_key1",
    "sb_key2",
    "sb_invis",
    "sb_invuln",
    "sb_suit",
    "sb_quad",
];

/// The sigil pictures, in the order of their bits.
const SIGIL_PICTURES: [&str; 4] = ["sb_sigil1", "sb_sigil2", "sb_sigil3", "sb_sigil4"];

/// The faces, from nearly dead to healthy, as `sb_faces`.
const FACES: [[&str; 2]; 5] = [
    ["face5", "face_p5"],
    ["face4", "face_p4"],
    ["face3", "face_p3"],
    ["face2", "face_p2"],
    ["face1", "face_p1"],
];

/// `cshift_water`.
const WATER_SHIFT: Shift = Shift::new([130, 80, 50], 128.0);
/// `cshift_slime`.
const SLIME_SHIFT: Shift = Shift::new([0, 25, 5], 150.0);
/// `cshift_lava`.
const LAVA_SHIFT: Shift = Shift::new([255, 80, 0], 150.0);

/// A line of the scores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Score {
    /// The player's name.
    pub name: String,
    /// Their frags.
    pub frags: i32,
    /// Their shirt in the high four bits, and pants in the low.
    pub colors: u8,
    /// Whether this is the player looking.
    pub own: bool,
}

/// What's over the view at the end of a level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Intermission {
    /// Nothing: the game is on.
    #[default]
    None,
    /// The level's results.
    Results,
    /// Text over the last view.
    Finale,
    /// Text alone.
    Cutscene,
}

impl Intermission {
    /// The screen the game means by `number`.
    #[must_use]
    pub fn from_number(number: u8) -> Self {
        match number {
            1 => Self::Results,
            2 => Self::Finale,
            3 => Self::Cutscene,
            _ => Self::None,
        }
    }
}

/// A colour over the view, from `cshift_t`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Shift {
    /// The colour.
    color: [u8; 3],
    /// How strong it is, out of 255.
    percent: f32,
}

impl Shift {
    /// A colour at a strength.
    const fn new(color: [u8; 3], percent: f32) -> Self {
        Self { color, percent }
    }
}

/// Text in the middle of the screen.
#[derive(Clone, Debug, Default, PartialEq)]
struct CenterText {
    /// The text.
    text: String,
    /// When it arrived.
    started: f64,
    /// When it goes.
    until: f64,
}

/// The status bar and everything else over the view.
#[derive(Clone, Debug, PartialEq)]
pub struct Hud {
    /// The latest stats.
    stats: [i32; STAT_COUNT],
    /// When each item was picked up.
    item_times: [f64; 32],
    /// Game time.
    time: f64,
    /// When the face stops showing pain.
    face_pain_until: f64,
    /// Being in a liquid.
    contents_shift: Shift,
    /// Being hurt.
    damage_shift: Shift,
    /// Picking something up.
    bonus_shift: Shift,
    /// The middle of the screen.
    center: CenterText,
    /// The end of level screen.
    intermission: Intermission,
    /// When the level ended.
    completed_time: f64,
    /// Whether the scores are held open.
    show_scores: bool,
    /// The level's title.
    level_title: String,
    /// Everyone's scores, best first.
    scores: Vec<Score>,
    /// The colours players can wear.
    shirts: [[u8; 3]; 16],
}

impl Default for Hud {
    fn default() -> Self {
        Self {
            stats: [0; STAT_COUNT],
            item_times: [0.0; 32],
            time: 0.0,
            face_pain_until: 0.0,
            contents_shift: Shift::default(),
            damage_shift: Shift::default(),
            bonus_shift: Shift::default(),
            center: CenterText::default(),
            intermission: Intermission::None,
            completed_time: 0.0,
            show_scores: false,
            level_title: String::new(),
            scores: Vec::new(),
            shirts: [[0; 3]; 16],
        }
    }
}

/// `number` as digits, the last `digits` of them, with a `-` for negative.
fn digits_of(number: i32, digits: usize) -> Vec<u8> {
    let text = number.to_string().into_bytes();
    let skip = text.len().saturating_sub(digits);
    text[skip..].to_vec()
}

/// Draws a big number whose right edge is `digits` places from `x`.
fn draw_number(canvas: &mut Canvas<'_>, at: (f32, f32), number: i32, digits: usize, alt: bool) {
    let shown = digits_of(number, digits);
    #[expect(clippy::cast_precision_loss, reason = "a few digits")]
    let mut x = at.0 + (digits - shown.len()) as f32 * DIGIT_WIDTH;
    let prefix = if alt { "anum" } else { "num" };
    for digit in shown {
        let name = if digit == b'-' {
            format!("{prefix}_minus")
        } else {
            format!("{prefix}_{}", char::from(digit))
        };
        canvas.picture(&name, x, at.1);
        x += DIGIT_WIDTH;
    }
}

impl Hud {
    /// A status bar for a new level, which keeps nothing from the last one.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The items held.
    #[expect(clippy::cast_sign_loss, reason = "the items are bits")]
    fn items(&self) -> u32 {
        self.stats[STAT_ITEMS] as u32
    }

    /// Takes the game's numbers at game time `time`, noting what's new.
    pub fn set_stats(&mut self, stats: &[i32], time: f64) {
        self.time = time;
        let before = self.items();
        for (stat, value) in self.stats.iter_mut().zip(stats) {
            *stat = *value;
        }
        let gained = self.items() & !before;
        for (bit, picked_up) in self.item_times.iter_mut().enumerate() {
            if gained & (1 << bit) != 0 {
                *picked_up = time;
            }
        }
    }

    /// Whether the player is dead, by the numbers the game sent.
    #[must_use]
    pub fn is_dead(&self) -> bool {
        self.stats[STAT_HEALTH] <= 0
    }

    /// Reddens the view and pains the face, from `V_ParseDamage`.
    pub fn damage(&mut self, armor: u8, blood: u8) {
        let count = (f32::from(blood) * 0.5 + f32::from(armor) * 0.5).max(10.0);
        self.face_pain_until = self.time + FACE_PAIN_SECONDS;
        self.damage_shift.percent = (self.damage_shift.percent + 3.0 * count).clamp(0.0, 150.0);
        self.damage_shift.color = if armor > blood {
            [200, 100, 100]
        } else if armor != 0 {
            [220, 50, 50]
        } else {
            [255, 0, 0]
        };
    }

    /// Flashes the view, from `V_BonusFlash_f`.
    pub fn bonus_flash(&mut self) {
        self.bonus_shift = Shift::new([215, 186, 69], 50.0);
    }

    /// Shows `text` in the middle of the screen, from `SCR_CenterPrint`.
    pub fn center_print(&mut self, text: String) {
        self.center = CenterText {
            text,
            started: self.time,
            until: self.time + CENTER_SECONDS,
        };
    }

    /// Starts an end of level screen.
    pub fn set_intermission(&mut self, intermission: Intermission) {
        self.intermission = intermission;
        self.completed_time = self.time;
    }

    /// The end of level screen showing.
    #[must_use]
    pub fn intermission(&self) -> Intermission {
        self.intermission
    }

    /// Sets the level's title.
    pub fn set_level_title(&mut self, title: String) {
        self.level_title = title;
    }

    /// The level's title.
    #[must_use]
    pub fn level_title(&self) -> &str {
        &self.level_title
    }

    /// The weapon in use, by its item bit.
    #[must_use]
    pub fn active_weapon(&self) -> i32 {
        self.stats[STAT_ACTIVE_WEAPON]
    }

    /// The ammo left for the weapon in use.
    #[must_use]
    pub fn ammo(&self) -> i32 {
        self.stats[STAT_AMMO]
    }

    /// The monsters killed and how many there are.
    #[must_use]
    pub fn monsters(&self) -> (i32, i32) {
        (self.stats[STAT_MONSTERS], self.stats[STAT_TOTAL_MONSTERS])
    }

    /// Takes the colours players can wear from `palette`, from
    /// `Sbar_ColorForMap`.
    pub fn set_palette(&mut self, palette: &Palette) {
        for (index, shirt) in (0u8..).zip(&mut self.shirts) {
            *shirt = palette.color(index * 16 + 8);
        }
    }

    /// Takes everyone's scores.
    pub fn set_scores(&mut self, mut scores: Vec<Score>) {
        scores.sort_by_key(|score| std::cmp::Reverse(score.frags));
        self.scores = scores;
    }

    /// The colour of `index` as the canvas wants it.
    fn shirt(&self, index: u8) -> [f32; 4] {
        let [red, green, blue] = self.shirts[usize::from(index & 15)].map(f32::from);
        [red / 255.0, green / 255.0, blue / 255.0, 1.0]
    }

    /// Draws everyone's name, frags and colours, from `Sbar_DeathmatchOverlay`.
    fn draw_ranking(&self, canvas: &mut Canvas<'_>) {
        let left = ((canvas.width() - 320.0) / 2.0).floor();
        let top = ((canvas.height() - 200.0) / 2.0).floor();
        if let Some((width, _height)) = canvas.size_of("gfx/ranking.lmp") {
            let x = left + ((320.0 - width) / 2.0).floor();
            canvas.picture("gfx/ranking.lmp", x, top + 8.0);
        }
        let x = left + 80.0;
        let mut y = top + 40.0;
        for score in &self.scores {
            canvas.fill([x, y, x + 40.0, y + 4.0], self.shirt(score.colors >> 4));
            canvas.fill([x, y + 4.0, x + 40.0, y + 8.0], self.shirt(score.colors));
            let frags = format!("{:3}", score.frags);
            canvas.text(x + 8.0, y, frags.as_bytes());
            if score.own {
                canvas.character(x - 8.0, y, 12);
            }
            canvas.text(x + 64.0, y, score.name.as_bytes());
            y += 10.0;
        }
    }

    /// Draws the best scores on the bar, from `Sbar_DrawFrags`.
    fn draw_frags(&self, canvas: &mut Canvas<'_>, x: f32, top: f32) {
        for (slot, score) in (0u8..).zip(self.scores.iter().take(4)) {
            let left = x + (23.0 + 4.0 * f32::from(slot)) * 8.0;
            canvas.fill(
                [left + 10.0, top + 1.0, left + 38.0, top + 5.0],
                self.shirt(score.colors >> 4),
            );
            canvas.fill(
                [left + 10.0, top + 5.0, left + 38.0, top + 8.0],
                self.shirt(score.colors),
            );
            let frags = format!("{:3}", score.frags);
            canvas.text(left + 12.0, top, frags.as_bytes());
            if score.own {
                canvas.character(left + 6.0, top, 16);
                canvas.character(left + 32.0, top, 17);
            }
        }
    }

    /// Draws scores beside the bar, around the player's own, from
    /// `Sbar_MiniDeathmatchOverlay`.
    fn draw_mini_ranking(&self, canvas: &mut Canvas<'_>, x: f32, y: f32) {
        if canvas.width() < MINI_RANKING_WIDTH {
            return;
        }
        let total = self.scores.len();
        let own = self.scores.iter().position(|score| score.own);
        let first = own.map_or(0, |own| own.saturating_sub(MINI_RANKING_LINES / 2));
        let first = first.min(total.saturating_sub(MINI_RANKING_LINES));
        let left = x + BAR_WIDTH + 4.0;
        let top = y - BAR_HEIGHT;
        let shown = self.scores.iter().skip(first).take(MINI_RANKING_LINES);
        for (line, score) in (0u8..).zip(shown) {
            let row = top + 8.0 * f32::from(line);
            canvas.fill(
                [left, row + 1.0, left + 40.0, row + 4.0],
                self.shirt(score.colors >> 4),
            );
            canvas.fill(
                [left, row + 4.0, left + 40.0, row + 8.0],
                self.shirt(score.colors),
            );
            let frags = format!("{:3}", score.frags);
            canvas.text(left + 8.0, row, frags.as_bytes());
            if score.own {
                canvas.character(left, row, 16);
                canvas.character(left + 32.0, row, 17);
            }
            canvas.text(left + 48.0, row, score.name.as_bytes());
        }
    }

    /// Holds the scores open, or lets them go.
    pub fn set_show_scores(&mut self, show: bool) {
        self.show_scores = show;
    }

    /// Lets flashes fade by `seconds`, with the eye in `contents`, from
    /// `V_UpdatePalette` and `V_SetContentsColor`.
    pub fn update_view(&mut self, seconds: f32, contents: Contents) {
        self.contents_shift = match contents {
            Contents::Empty | Contents::Solid | Contents::Sky => Shift::default(),
            Contents::Lava => LAVA_SHIFT,
            Contents::Slime => SLIME_SHIFT,
            Contents::Water => WATER_SHIFT,
        };
        self.damage_shift.percent = (self.damage_shift.percent - seconds * 150.0).max(0.0);
        self.bonus_shift.percent = (self.bonus_shift.percent - seconds * 100.0).max(0.0);
    }

    /// The colour of a powerup, from `V_CalcPowerupCshift`.
    fn powerup_shift(&self) -> Shift {
        let items = self.items();
        if items & IT_QUAD != 0 {
            Shift::new([0, 0, 255], 30.0)
        } else if items & IT_SUIT != 0 {
            Shift::new([0, 255, 0], 20.0)
        } else if items & IT_INVISIBILITY != 0 {
            Shift::new([100, 100, 100], 100.0)
        } else if items & IT_INVULNERABILITY != 0 {
            Shift::new([255, 255, 0], 30.0)
        } else {
            Shift::default()
        }
    }

    /// The colour over the view, from `V_CalcBlend`.
    #[must_use]
    pub fn tint(&self) -> [f32; 4] {
        let (mut red, mut green, mut blue, mut alpha) = (0.0, 0.0, 0.0, 0.0);
        for shift in [
            self.contents_shift,
            self.damage_shift,
            self.bonus_shift,
            self.powerup_shift(),
        ] {
            let strength = shift.percent / 255.0;
            if strength == 0.0 {
                continue;
            }
            alpha += strength * (1.0 - alpha);
            let share: f32 = strength / alpha;
            let [r, g, b] = shift.color.map(f32::from);
            red = red * (1.0 - share) + r * share;
            green = green * (1.0 - share) + g * share;
            blue = blue * (1.0 - share) + b * share;
        }
        [
            red / 255.0,
            green / 255.0,
            blue / 255.0,
            f32::clamp(alpha, 0.0, 1.0),
        ]
    }

    /// Fills the view with its tint.
    pub fn draw_tint(&self, canvas: &mut Canvas<'_>) {
        let tint = self.tint();
        if tint[3] > 0.0 {
            let place = [0.0, 0.0, canvas.width(), canvas.height()];
            canvas.fill(place, tint);
        }
    }

    /// Draws what's over the game itself. `playing` says no menu or console is
    /// over it, which the end of level screens wait for.
    pub fn draw(&self, canvas: &mut Canvas<'_>, playing: bool) {
        match self.intermission {
            Intermission::Results if playing && self.stats[STAT_DEATHMATCH] != 0 => {
                self.draw_ranking(canvas);
            }
            Intermission::Results if playing => self.draw_results(canvas),
            Intermission::Finale if playing => {
                Self::draw_finale_title(canvas);
                self.draw_center(canvas);
            }
            Intermission::Cutscene if playing => self.draw_center(canvas),
            _ => {
                if playing {
                    self.draw_center(canvas);
                }
                self.draw_bar(canvas);
            }
        }
    }

    /// Draws the words in the middle of the screen, from `SCR_DrawCenterString`.
    fn draw_center(&self, canvas: &mut Canvas<'_>) {
        let holding = self.intermission != Intermission::None;
        if self.center.text.is_empty() || (!holding && self.time >= self.center.until) {
            return;
        }
        // The finale types its words out.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a count of characters"
        )]
        let mut remaining = if holding {
            (PRINT_SPEED * (self.time - self.center.started)).max(0.0) as usize
        } else {
            usize::MAX
        };
        let lines: Vec<&[u8]> = self.center.text.as_bytes().split(|&c| c == b'\n').collect();
        let mut y = if lines.len() <= 4 {
            canvas.height() * 0.35
        } else {
            LONG_MESSAGE_TOP
        };
        for line in lines {
            let line = line.get(..MAX_CENTER_LINE).unwrap_or(line);
            #[expect(clippy::cast_precision_loss, reason = "a short line")]
            let mut x = (canvas.width() - line.len() as f32 * CHARACTER_SIZE) / 2.0;
            for &code in line {
                canvas.character(x, y, code);
                if remaining == 0 {
                    return;
                }
                remaining -= 1;
                x += CHARACTER_SIZE;
            }
            y += CHARACTER_SIZE;
        }
    }

    /// Where the bar's left and top edges are.
    fn bar_origin(canvas: &Canvas<'_>) -> (f32, f32) {
        (
            ((canvas.width() - BAR_WIDTH) / 2.0).floor(),
            canvas.height() - BAR_HEIGHT,
        )
    }

    /// Draws the bar, from `Sbar_Draw`.
    fn draw_bar(&self, canvas: &mut Canvas<'_>) {
        let (x, y) = Self::bar_origin(canvas);
        self.draw_inventory(canvas, x, y);
        if self.scores.len() > 1 {
            self.draw_frags(canvas, x, y - BAR_HEIGHT);
        }
        if self.stats[STAT_DEATHMATCH] != 0 {
            self.draw_mini_ranking(canvas, x, y);
        }
        if self.show_scores || self.is_dead() {
            canvas.picture("scorebar", x, y);
            self.draw_solo_scores(canvas, x, y);
            if self.stats[STAT_DEATHMATCH] != 0 {
                self.draw_ranking(canvas);
            }
            return;
        }
        canvas.picture("sbar", x, y);
        let items = self.items();
        let armor = self.stats[STAT_ARMOR];
        if items & IT_INVULNERABILITY != 0 {
            draw_number(canvas, (x + 24.0, y), 666, 3, true);
            canvas.picture("disc", x, y);
        } else {
            draw_number(canvas, (x + 24.0, y), armor, 3, armor <= 25);
            let picture = if items & IT_ARMOR3 != 0 {
                Some("sb_armor3")
            } else if items & IT_ARMOR2 != 0 {
                Some("sb_armor2")
            } else if items & IT_ARMOR1 != 0 {
                Some("sb_armor1")
            } else {
                None
            };
            if let Some(picture) = picture {
                canvas.picture(picture, x, y);
            }
        }
        canvas.picture(self.face(), x + 112.0, y);
        let health = self.stats[STAT_HEALTH];
        draw_number(canvas, (x + 136.0, y), health, 3, health <= 25);
        let ammo_picture = if items & IT_SHELLS != 0 {
            Some("sb_shells")
        } else if items & IT_NAILS != 0 {
            Some("sb_nails")
        } else if items & IT_ROCKETS != 0 {
            Some("sb_rocket")
        } else if items & IT_CELLS != 0 {
            Some("sb_cells")
        } else {
            None
        };
        if let Some(picture) = ammo_picture {
            canvas.picture(picture, x + 224.0, y);
        }
        let ammo = self.stats[STAT_AMMO];
        draw_number(canvas, (x + 248.0, y), ammo, 3, ammo <= 10);
    }

    /// The face to show, from `Sbar_DrawFace`.
    fn face(&self) -> &'static str {
        let items = self.items();
        let both = IT_INVISIBILITY | IT_INVULNERABILITY;
        if items & both == both {
            "face_inv2"
        } else if items & IT_QUAD != 0 {
            "face_quad"
        } else if items & IT_INVISIBILITY != 0 {
            "face_invis"
        } else if items & IT_INVULNERABILITY != 0 {
            "face_invul2"
        } else {
            let health = self.stats[STAT_HEALTH];
            let row = if health >= 100 { 4 } else { health / 20 };
            let row = usize::try_from(row.clamp(0, 4)).unwrap_or(0);
            FACES[row][usize::from(self.time <= self.face_pain_until)]
        }
    }

    /// Draws the weapons, ammo and items, from `Sbar_DrawInventory`.
    fn draw_inventory(&self, canvas: &mut Canvas<'_>, x: f32, y: f32) {
        let top = y - BAR_HEIGHT;
        canvas.picture("ibar", x, top);
        let items = self.items();
        let active = self.stats[STAT_ACTIVE_WEAPON];
        for (slot, name) in (0u8..).zip(WEAPON_PICTURES) {
            let bit = IT_SHOTGUN << slot;
            if items & bit == 0 {
                continue;
            }
            let held = (self.time - self.item_times[usize::from(slot)]) * 10.0;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "tenths of a second, counted from zero"
            )]
            let picture = if held >= 10.0 {
                let selected = active == i32::try_from(bit).unwrap_or(0);
                format!("{}_{name}", if selected { "inv2" } else { "inv" })
            } else {
                format!("inva{}_{name}", held.max(0.0) as u32 % 5 + 1)
            };
            canvas.picture(&picture, x + f32::from(slot) * 24.0, top + 8.0);
        }
        for (index, count) in (0u8..).zip(&self.stats[STAT_SHELLS..STAT_SHELLS + 4]) {
            let shown = format!("{:3}", count.rem_euclid(1000));
            for (column, digit) in (0u8..).zip(shown.bytes()) {
                if digit != b' ' {
                    let left = f32::from(index * 6 + column + 1) * 8.0 + 2.0;
                    canvas.character(x + left, top, 18 + digit - b'0');
                }
            }
        }
        for (slot, name) in (0u8..).zip(ITEM_PICTURES) {
            if items & (1 << (FIRST_ITEM_BIT + u32::from(slot))) != 0 {
                canvas.picture(name, x + 192.0 + f32::from(slot) * 16.0, top + 8.0);
            }
        }
        for (slot, name) in (0u8..).zip(SIGIL_PICTURES) {
            if items & (1 << (FIRST_SIGIL_BIT + u32::from(slot))) != 0 {
                canvas.picture(name, x + 288.0 + f32::from(slot) * 8.0, top + 8.0);
            }
        }
    }

    /// Minutes, tens and units of seconds of `time`.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a level lasts minutes"
    )]
    fn clock(time: f64) -> (i32, u32, u32) {
        let seconds = time.max(0.0) as u32;
        let minutes = i32::try_from(seconds / 60).unwrap_or(0);
        (minutes, seconds % 60 / 10, seconds % 10)
    }

    /// Draws the level's progress on the bar, from `Sbar_SoloScoreboard`.
    fn draw_solo_scores(&self, canvas: &mut Canvas<'_>, x: f32, y: f32) {
        let monsters = format!(
            "Monsters:{:3} /{:3}",
            self.stats[STAT_MONSTERS], self.stats[STAT_TOTAL_MONSTERS]
        );
        canvas.text(x + 8.0, y + 4.0, monsters.as_bytes());
        let secrets = format!(
            "Secrets :{:3} /{:3}",
            self.stats[STAT_SECRETS], self.stats[STAT_TOTAL_SECRETS]
        );
        canvas.text(x + 8.0, y + 12.0, secrets.as_bytes());
        let (minutes, tens, units) = Self::clock(self.time);
        let time = format!("Time :{minutes:3}:{tens}{units}");
        canvas.text(x + 184.0, y + 4.0, time.as_bytes());
        #[expect(clippy::cast_precision_loss, reason = "a short title")]
        let left = 232.0 - self.level_title.len() as f32 * 4.0;
        canvas.text(x + left, y + 12.0, self.level_title.as_bytes());
    }

    /// Draws the level's results, from `Sbar_IntermissionOverlay`, in a
    /// space of 320 by 200 in the middle of the screen.
    fn draw_results(&self, canvas: &mut Canvas<'_>) {
        let x = ((canvas.width() - 320.0) / 2.0).floor();
        let y = ((canvas.height() - 200.0) / 2.0).floor();
        canvas.picture("gfx/complete.lmp", x + 64.0, y + 24.0);
        canvas.picture("gfx/inter.lmp", x, y + 56.0);
        let (minutes, tens, units) = Self::clock(self.completed_time);
        draw_number(canvas, (x + 160.0, y + 64.0), minutes, 3, false);
        canvas.picture("num_colon", x + 234.0, y + 64.0);
        canvas.picture(&format!("num_{tens}"), x + 246.0, y + 64.0);
        canvas.picture(&format!("num_{units}"), x + 266.0, y + 64.0);
        let rows = [
            (104.0, STAT_SECRETS, STAT_TOTAL_SECRETS),
            (144.0, STAT_MONSTERS, STAT_TOTAL_MONSTERS),
        ];
        for (top, found, total) in rows {
            draw_number(canvas, (x + 160.0, y + top), self.stats[found], 3, false);
            canvas.picture("num_slash", x + 232.0, y + top);
            draw_number(canvas, (x + 240.0, y + top), self.stats[total], 3, false);
        }
    }

    /// Draws the title of the end of the game, from `Sbar_FinaleOverlay`.
    fn draw_finale_title(canvas: &mut Canvas<'_>) {
        if let Some((width, _height)) = canvas.size_of("gfx/finale.lmp") {
            let x = ((canvas.width() - width) / 2.0).floor();
            canvas.picture("gfx/finale.lmp", x, 16.0);
        }
    }
}
