//! A simulation for tests.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::Map;
use engine_script::Game;
use engine_sim::Simulation;

/// What every game here says about itself.
const GAME_RULES: &str = r#"
    set_number_fields({ "lip", "speed" })
    set_player_defaults({ classname = "player" })
"#;

/// A game running `source`, and its simulation.
pub(crate) fn game(source: &str) -> (Game, Rc<RefCell<Simulation>>) {
    let map = Arc::new(Map::default());
    let simulation = Rc::new(RefCell::new(Simulation::new(map, Vec3::ZERO)));
    let sources = [("rules", GAME_RULES), ("test", source)];
    let game = Game::new(&simulation, &sources).expect("scripts load");
    (game, simulation)
}
