//! A simulation for tests.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::Map;
use engine_script::Game;
use engine_sim::Simulation;

/// A game running `source`, and its simulation.
pub(crate) fn game(source: &str) -> (Game, Rc<RefCell<Simulation>>) {
    let map = Arc::new(Map::default());
    let simulation = Rc::new(RefCell::new(Simulation::new(map, Vec3::ZERO)));
    let game = Game::new(&simulation, &[("test", source)]).expect("scripts load");
    (game, simulation)
}
