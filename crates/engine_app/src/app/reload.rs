//! Reading effects again when their files change (proposal 0146).

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use engine_assets::FileSystem;
use engine_compile::{effect_files, read_effects};
use engine_map::Map;

use super::World;

/// How often the files are looked at.
const LOOK_EVERY: Duration = Duration::from_millis(500);

/// Looks at the files effects come from, on a thread of its own, and says when they change. What
/// ships has no dates to look at, so it stops, and it stops when nobody's listening.
pub(super) fn watch_effects(filesystem: &Arc<FileSystem>) -> Receiver<()> {
    let (tell, told) = mpsc::channel();
    let filesystem = Arc::clone(filesystem);
    let _watcher = std::thread::spawn(move || {
        let mut seen = effect_files(&filesystem);
        while !seen.is_empty() {
            std::thread::sleep(LOOK_EVERY);
            let now = effect_files(&filesystem);
            if now != seen {
                seen = now;
                if tell.send(()).is_err() {
                    return;
                }
            }
        }
    });
    told
}

impl World {
    /// Reads effects again if their files have changed. If they won't read, what's there stays,
    /// and that's said.
    pub(super) fn follow_effect_files(&mut self) {
        let changed = self
            .effect_changes
            .as_ref()
            .is_some_and(|told| told.try_iter().count() > 0);
        let Some(map) = self.map.clone().filter(|_| changed) else {
            return;
        };
        let read = match read_effects(&self.filesystem, &map) {
            Ok(read) => read,
            Err(problem) => {
                self.load_warning = Some(format!("effects stay as they were: {problem}"));
                return;
            }
        };
        self.effects.authored_mut().change_effects(&read.effects);
        let mut told = vec!["effects read again".to_owned()];
        for name in &read.missing_meshes {
            told.push(format!("{name} shows when the level's loaded again"));
        }
        if let Some(renderer) = &mut self.renderer {
            let held = Map {
                effects: read.effects,
                effect_pictures: read.pictures,
                effect_shaders: read.shaders,
                effect_motions: read.motions,
                fields: map.fields.clone(),
                ..Map::default()
            };
            if let Err(problem) = renderer.reload_effects(&held) {
                told.push(problem.to_string());
            }
            told.extend(renderer.take_shader_problems());
        }
        self.load_warning = Some(told.join("\n"));
    }
}
