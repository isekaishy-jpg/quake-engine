//! Keeping detailed light in the atlas for the tiles near the view (proposal
//! 0095).

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use engine_core::math::Vec3;
use engine_map::{Map, TileLight};

use crate::error::RenderError;
use crate::renderer::Renderer;

/// How many tiles from the view detail is wanted. Two seconds at top speed,
/// with tiles of 2048 units.
const SHOW_REACH: i32 = 2;

/// How many tiles from the view detail is kept, so going back and forth over an
/// edge doesn't read the same tile again and again.
const KEEP_REACH: i32 = 3;

/// How many tiles a frame copies to the GPU.
const SHOWN_PER_FRAME: usize = 2;

/// How many tiles are read ahead of being shown.
const MOST_ASKED: usize = 8;

/// Reads tiles off the main thread, and shows them as they arrive.
pub struct TileStreamer {
    /// The map.
    map: Arc<Map>,
    /// Asks for a tile.
    asks: Sender<u32>,
    /// What's been read.
    answers: Receiver<(u32, Option<TileLight>)>,
    /// What's been asked for and hasn't come.
    asked: HashSet<u32>,
    /// What's in the atlas.
    shown: HashSet<u32>,
    /// What the atlas had no room for, until something leaves.
    refused: HashSet<u32>,
}

impl TileStreamer {
    /// A streamer for `map`, or `None` if its light doesn't stream.
    #[must_use]
    pub fn new(map: &Arc<Map>) -> Option<Self> {
        map.light_tiles.as_ref()?;
        let (asks, asked) = channel::<u32>();
        let (answer, answers) = channel();
        let reader = Arc::clone(map);
        thread::spawn(move || {
            while let Ok(tile) = asked.recv() {
                let light = reader.light_tiles.as_ref().and_then(|t| t.read(tile));
                if answer.send((tile, light)).is_err() {
                    return;
                }
            }
        });
        Some(Self {
            map: Arc::clone(map),
            asks,
            answers,
            asked: HashSet::new(),
            shown: HashSet::new(),
            refused: HashSet::new(),
        })
    }

    /// Whether this is streaming `map`.
    #[must_use]
    pub fn is_for(&self, map: &Arc<Map>) -> bool {
        Arc::ptr_eq(&self.map, map)
    }

    /// How many tiles are in the atlas.
    #[must_use]
    pub fn shown(&self) -> usize {
        self.shown.len()
    }

    /// Whether anything asked for is still on its way.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.asked.is_empty()
    }

    /// Brings detail to the tiles near `eye`, and takes it from those far off.
    /// A frame's worth: call this every frame.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub fn update(&mut self, eye: Vec3, renderer: &mut Renderer) -> Result<(), RenderError> {
        let map = Arc::clone(&self.map);
        let Some(tiles) = map.light_tiles.as_ref() else {
            return Ok(());
        };
        // A map loaded again, as after losing the device, starts with nothing.
        if renderer.shown_tiles() != self.shown.len() {
            for tile in self.shown.drain() {
                tiles.hold(tile, None);
            }
        }
        let keep: HashSet<u32> = tiles.tiles_near(eye, KEEP_REACH).into_iter().collect();
        let leaving: Vec<u32> = self.shown.difference(&keep).copied().collect();
        for tile in leaving {
            renderer.hide_tile_light(tile)?;
            tiles.hold(tile, None);
            self.shown.remove(&tile);
            self.refused.clear();
        }
        for _ in 0..SHOWN_PER_FRAME {
            let Ok((tile, light)) = self.answers.try_recv() else {
                break;
            };
            self.asked.remove(&tile);
            let Some(light) = light.filter(|_| keep.contains(&tile)) else {
                continue;
            };
            if renderer.show_tile_light(tile, &light)? {
                tiles.hold(tile, Some(Arc::new(light)));
                self.shown.insert(tile);
            } else {
                self.refused.insert(tile);
            }
        }
        for tile in tiles.tiles_near(eye, SHOW_REACH) {
            let waiting = self.shown.contains(&tile)
                || self.asked.contains(&tile)
                || self.refused.contains(&tile);
            if !waiting && self.asked.len() < MOST_ASKED && self.asks.send(tile).is_ok() {
                self.asked.insert(tile);
            }
        }
        Ok(())
    }
}
