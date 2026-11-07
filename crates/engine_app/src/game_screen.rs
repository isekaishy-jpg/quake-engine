//! What's going on in the game, for what's drawn over it (proposal 0139). The UI script draws
//! it. This only keeps it.

use std::collections::BTreeMap;

use fixture_quake::{ITEM_NAMES, STAT_ACTIVE_WEAPON, STAT_AMMO, STAT_ARMOR, STAT_COUNT};
use fixture_quake::{STAT_HEALTH, STAT_ITEMS, STAT_MONSTERS, STAT_NAMES, STAT_TOTAL_MONSTERS};

/// How long words stay in the middle of the screen, from `scr_centertime`.
const CENTER_SECONDS: f64 = 2.0;

/// What's over the view at the end of a level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Intermission {
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
    pub(crate) const fn from_number(number: u8) -> Self {
        match number {
            1 => Self::Results,
            2 => Self::Finale,
            3 => Self::Cutscene,
            _ => Self::None,
        }
    }

    /// What the UI script calls it.
    const fn name(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Results => "results",
            Self::Finale => "finale",
            Self::Cutscene => "cutscene",
        }
    }
}

/// A line of the scores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Score {
    /// The player's name.
    pub(crate) name: String,
    /// Their frags.
    pub(crate) frags: i32,
    /// Their shirt in the high four bits, and pants in the low.
    pub(crate) colors: u8,
    /// Whether this is the player looking.
    pub(crate) own: bool,
}

/// What's going on in the game.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GameScreen {
    /// The latest stats.
    stats: [i32; STAT_COUNT],
    /// Game time.
    time: f64,
    /// Words for the middle of the screen.
    center: String,
    /// When they came.
    center_started: f64,
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
}

impl GameScreen {
    /// Takes the game's numbers at game time `time`.
    pub(crate) fn set_stats(&mut self, stats: &[i32], time: f64) {
        self.time = time;
        for (stat, value) in self.stats.iter_mut().zip(stats) {
            *stat = *value;
        }
    }

    /// Whether the player is dead, by the numbers the game sent.
    pub(crate) const fn is_dead(&self) -> bool {
        self.stats[STAT_HEALTH] <= 0
    }

    /// Shows `text` in the middle of the screen.
    pub(crate) fn center_print(&mut self, text: String) {
        self.center = text;
        self.center_started = self.time;
    }

    /// Starts an end of level screen.
    pub(crate) const fn set_intermission(&mut self, intermission: Intermission) {
        self.intermission = intermission;
        self.completed_time = self.time;
    }

    /// The end of level screen.
    pub(crate) const fn intermission(&self) -> Intermission {
        self.intermission
    }

    /// Sets the level's title.
    pub(crate) fn set_level_title(&mut self, title: String) {
        self.level_title = title;
    }

    /// The level's title.
    pub(crate) fn level_title(&self) -> &str {
        &self.level_title
    }

    /// The weapon in use.
    pub(crate) const fn active_weapon(&self) -> i32 {
        self.stats[STAT_ACTIVE_WEAPON]
    }

    /// The ammo left for the weapon in use.
    pub(crate) const fn ammo(&self) -> i32 {
        self.stats[STAT_AMMO]
    }

    /// The monsters killed and how many there are.
    pub(crate) const fn monsters(&self) -> (i32, i32) {
        (self.stats[STAT_MONSTERS], self.stats[STAT_TOTAL_MONSTERS])
    }

    /// Takes everyone's scores.
    pub(crate) fn set_scores(&mut self, mut scores: Vec<Score>) {
        scores.sort_by_key(|score| std::cmp::Reverse(score.frags));
        self.scores = scores;
    }

    /// Says whether the scores are held open.
    pub(crate) const fn set_show_scores(&mut self, show: bool) {
        self.show_scores = show;
    }

    /// Everyone's scores, best first.
    pub(crate) fn scores(&self) -> &[Score] {
        &self.scores
    }

    /// The stats and what's held, by name, for the UI script.
    pub(crate) fn stats_by_name(&self) -> BTreeMap<String, f64> {
        let mut named: BTreeMap<String, f64> = STAT_NAMES
            .iter()
            .map(|(name, stat)| ((*name).to_owned(), f64::from(self.stats[*stat])))
            .collect();
        named.insert("armor".to_owned(), f64::from(self.stats[STAT_ARMOR]));
        let items = self.stats[STAT_ITEMS];
        for (name, bit) in ITEM_NAMES {
            named.insert(name.to_owned(), f64::from(u8::from(items & bit != 0)));
        }
        named
    }

    /// The rest of what's going on, as numbers.
    pub(crate) fn numbers(&self) -> BTreeMap<String, f64> {
        let left = self.center_started + CENTER_SECONDS - self.time;
        BTreeMap::from([
            ("game_time".to_owned(), self.time),
            ("completed_time".to_owned(), self.completed_time),
            ("center_age".to_owned(), self.time - self.center_started),
            ("center_left".to_owned(), left),
            (
                "show_scores".to_owned(),
                f64::from(u8::from(self.show_scores)),
            ),
        ])
    }

    /// What's going on, as text.
    pub(crate) fn texts(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("center".to_owned(), self.center.clone()),
            ("level_title".to_owned(), self.level_title.clone()),
            (
                "intermission".to_owned(),
                self.intermission.name().to_owned(),
            ),
        ])
    }
}
