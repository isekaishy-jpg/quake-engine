//! The Luau host, with small scripts.

mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use common::game;
use engine_core::math::Vec3;
use engine_map::{AttachPoint, Entity, Map, Model};
use engine_script::Game;
use engine_sim::{Callback, EntityId, Simulation};

/// A map entity from pairs.
fn entity(fields: &[(&str, &str)]) -> Entity {
    Entity {
        fields: fields
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect(),
    }
}

/// The last entity spawned.
fn last(simulation: &Rc<RefCell<Simulation>>) -> EntityId {
    *simulation.borrow().ids().last().expect("an entity")
}

/// What the scripts printed.
fn prints(game: &Game) -> Vec<String> {
    game.take_outbox()
        .prints
        .into_iter()
        .map(|(_target, text)| text)
        .collect()
}

#[test]
fn a_map_entity_spawns_through_the_function_named_after_its_class() {
    let (game, simulation) = game(
        r#"
        function thing(self)
            engine.print(self.message .. " " .. self.speed)
            self.nextthink = 5
        end
        "#,
    );
    let fields = [
        ("classname", "thing"),
        ("origin", "1 2 3"),
        ("message", "hi"),
        ("speed", "40"),
    ];
    game.spawn_map(&[entity(&fields)]);
    assert_eq!(prints(&game), ["hi 40"]);
    let id = last(&simulation);
    let simulation = simulation.borrow();
    let spawned = simulation.entity(id).expect("spawned");
    assert_eq!(spawned.origin, Vec3::new(1.0, 2.0, 3.0));
    assert!((spawned.next_think - 5.0).abs() < f64::EPSILON);
}

#[test]
fn an_unknown_class_is_removed() {
    let (game, simulation) = game("");
    game.spawn_map(&[entity(&[("classname", "nothing_here")])]);
    assert_eq!(simulation.borrow().ids().len(), 1, "only the player");
}

#[test]
fn angle_becomes_a_yaw_and_missing_numbers_read_as_zero() {
    let (game, _simulation) = game(
        r#"
        function thing(self)
            engine.print(self.angles.y .. " " .. self.lip)
        end
        "#,
    );
    game.spawn_map(&[entity(&[("classname", "thing"), ("angle", "90")])]);
    assert_eq!(prints(&game), ["90 0"]);
}

#[test]
fn callbacks_reach_the_right_entity_with_their_arguments() {
    let (game, simulation) = game(
        r#"
        function thing(self)
            self.name = "door"
            self.think = function(me) engine.print(me.name .. " thinks") end
            self.touch = function(me, other) engine.print(me.name .. " touched by " .. other.classname) end
            self.blocked = function(me, other) engine.print(me.name .. " blocked by " .. other.classname) end
        end
        "#,
    );
    game.spawn_map(&[entity(&[("classname", "thing")])]);
    let id = last(&simulation);
    let other = simulation.borrow().player_id();
    game.run(Callback::Think(id, 0.0));
    game.run(Callback::Touch {
        id,
        other: Some(other),
    });
    game.run(Callback::Blocked { id, other });
    assert_eq!(
        prints(&game),
        [
            "door thinks",
            "door touched by player",
            "door blocked by player"
        ]
    );
}

#[test]
fn a_missing_callback_is_fine() {
    let (game, simulation) = game("function thing(self) end");
    game.spawn_map(&[entity(&[("classname", "thing")])]);
    game.run(Callback::Think(last(&simulation), 0.0));
    assert!(prints(&game).is_empty());
}

#[test]
fn engine_fields_write_through_to_the_simulation() {
    let (game, simulation) = game(
        r#"
        function thing(self)
            self.velocity = vector.create(1, 2, 3)
            self.solid = 4
            self.movetype = 7
        end
        "#,
    );
    game.spawn_map(&[entity(&[("classname", "thing")])]);
    let id = last(&simulation);
    let simulation = simulation.borrow();
    let spawned = simulation.entity(id).expect("spawned");
    assert_eq!(spawned.velocity, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(spawned.solid, engine_sim::Solid::Bsp);
    assert_eq!(spawned.move_type, engine_sim::MoveType::Push);
}

#[test]
fn find_goes_in_slot_order_and_removed_entities_are_gone() {
    let (game, _simulation) = game(
        r#"
        function thing(self) end
        "#,
    );
    let fields = [("classname", "thing"), ("targetname", "t")];
    game.spawn_map(&[entity(&fields), entity(&fields)]);
    game.exec(
        r#"
        local first = find(nil, "targetname", "t")
        local second = find(first, "targetname", "t")
        engine.print(tostring(first ~= second))
        engine.print(tostring(find(second, "targetname", "t")))
        remove(first)
        engine.print(tostring(#find_all("targetname", "t")))
        "#,
    )
    .expect("runs");
    assert_eq!(prints(&game), ["true", "nil", "1"]);
}

#[test]
fn a_script_error_is_reported_and_the_game_carries_on() {
    let (game, simulation) = game(
        r#"
        function thing(self)
            self.think = function(me) error("broken") end
        end
        "#,
    );
    game.spawn_map(&[entity(&[("classname", "thing")])]);
    game.run(Callback::Think(last(&simulation), 0.0));
    let output = prints(&game);
    assert!(output[0].contains("broken"), "{output:?}");
    game.exec("engine.print('still here')").expect("runs");
    assert_eq!(prints(&game), ["still here"]);
}

#[test]
fn a_runaway_script_is_stopped() {
    let (game, simulation) = game(
        r#"
        function thing(self)
            self.think = function(me) while true do end end
        end
        "#,
    );
    game.spawn_map(&[entity(&[("classname", "thing")])]);
    game.run(Callback::Think(last(&simulation), 0.0));
    assert!(prints(&game)[0].contains("too long"));
}

#[test]
fn scripts_have_no_way_to_the_operating_system() {
    let (game, _simulation) = game("");
    game.exec(
        r#"
        engine.print(tostring(os.execute) .. " " .. tostring(io) .. " " .. tostring(require))
        engine.print(tostring(loadstring))
        "#,
    )
    .expect("runs");
    assert_eq!(prints(&game), ["nil nil nil", "nil"]);
    assert!(game.exec(r#"require("./Cargo")"#).is_err());
}

#[test]
fn libraries_are_read_only() {
    let (game, _simulation) = game("");
    assert!(game.exec("math.pi = 3").is_err());
}

#[test]
fn a_script_that_will_not_compile_is_refused() {
    let map = Arc::new(Map::default());
    let simulation = Rc::new(RefCell::new(Simulation::new(map, Vec3::ZERO)));
    assert!(Game::new(&simulation, &[("bad", "function (")]).is_err());
}

#[test]
fn the_player_is_an_entity_scripts_can_move() {
    let (game, simulation) = game("");
    game.exec(
        r#"
        local player = find(nil, "classname", "player")
        player.origin = vector.create(5, 6, 7)
        player.velocity = vector.create(0, 0, 100)
        "#,
    )
    .expect("runs");
    let simulation = simulation.borrow();
    assert_eq!(simulation.player().origin, Vec3::new(5.0, 6.0, 7.0));
    assert_eq!(simulation.player().velocity, Vec3::new(0.0, 0.0, 100.0));
}

#[test]
fn a_sound_comes_from_the_middle_of_its_entity() {
    let (game, _simulation) = game("");
    game.exec(
        r#"
        local thing = spawn()
        thing.origin = vector.create(100, 0, 0)
        setsize(thing, vector.create(-10, -10, -10), vector.create(30, 10, 10))
        sound(thing, 2, "misc/talk.wav", 0.5, 1)
        "#,
    )
    .expect("runs");
    let outbox = game.take_outbox();
    assert_eq!(outbox.sounds.len(), 1);
    let sound = &outbox.sounds[0];
    assert_eq!(sound.origin, Vec3::new(110.0, 0.0, 0.0));
    assert_eq!(sound.channel, 2);
    assert_eq!(sound.name, "misc/talk.wav");
    assert!((sound.volume - 0.5).abs() < f32::EPSILON);
    assert!(!sound.own);
}

#[test]
fn a_sound_with_no_sample_is_ignored() {
    let (game, _simulation) = game("");
    game.exec(r#"sound(spawn(), 2, nil, 1, 1) sound(spawn(), 2, "", 1, 1)"#)
        .expect("runs");
    assert!(game.take_outbox().sounds.is_empty());
}

#[test]
fn sound_values_out_of_range_are_refused() {
    let (game, _simulation) = game("");
    assert!(game.exec(r#"sound(spawn(), 2, "a.wav", 2, 1)"#).is_err());
    assert!(game.exec(r#"sound(spawn(), 2, "a.wav", 1, 5)"#).is_err());
    assert!(game.exec(r#"sound(spawn(), 8, "a.wav", 1, 1)"#).is_err());
    assert!(
        game.exec(r#"ambientsound(vector.zero, "a.wav", 2, 1)"#)
            .is_err()
    );
}

#[test]
fn random_is_the_same_each_run_and_stays_in_range() {
    let draw = || {
        let (game, _simulation) = game(
            r#"
            function draw()
                local value = random()
                assert(value >= 0 and value < 1)
                engine.print(tostring(value))
            end
            "#,
        );
        game.exec("for i = 1, 100 do draw() end").expect("runs");
        prints(&game)
    };
    assert_eq!(draw(), draw());
}

#[test]
fn player_fields_are_nil_on_anything_else() {
    let (game, _simulation) = game("");
    game.exec(
        r#"
        local player = find_all("classname", "player")[1]
        assert(player.waterlevel == 0)
        assert(player.onground == false)
        assert(spawn().waterlevel == nil)
        "#,
    )
    .expect("runs");
}

#[test]
fn a_script_sees_what_another_script_writes_to_a_global() {
    let map = Arc::new(Map::default());
    let simulation = Rc::new(RefCell::new(Simulation::new(map, Vec3::ZERO)));
    let sources = [
        ("reader", "shared = 1 function read() return nil end"),
        ("writer", "function write(value) shared = value end"),
        (
            "other",
            "function show() engine.print(tostring(shared)) end",
        ),
    ];
    let game = Game::new(&simulation, &sources).expect("scripts load");
    game.exec("write(5) show()").expect("runs");
    assert_eq!(prints(&game), ["5"]);
}

#[test]
fn a_trace_result_reaches_the_script_that_asked() {
    let (game, _simulation) = game(
        r#"
        function report()
            traceline(vector.zero, vector.create(10, 0, 0), true, nil)
            engine.print(tostring(trace_endpos))
        end
        "#,
    );
    game.exec("report()").expect("runs");
    assert_eq!(prints(&game), ["10, 0, 0"]);
}

/// A game on a map with a named place, and a model with a place on it.
fn attaching_game(script: &str) -> (Game, Rc<RefCell<Simulation>>) {
    let mut map = Map {
        models: vec![Model::default(), Model::default()],
        ..Map::default()
    };
    let at = |x: f32| Vec3::new(x, 0.0, 0.0);
    map.places.push(AttachPoint {
        name: "door".to_owned(),
        origin: at(5.0),
        angles: Vec3::new(0.0, 90.0, 0.0),
    });
    let spout = AttachPoint {
        name: "spout".to_owned(),
        origin: at(7.0),
        ..AttachPoint::default()
    };
    map.model_points.push((1, spout));
    let simulation = Rc::new(RefCell::new(Simulation::new(Arc::new(map), Vec3::ZERO)));
    let game = Game::new(&simulation, &[("test", script)]).expect("runs");
    (game, simulation)
}

/// What every attaching test spawns from.
const CLASSES: &str = r#"
    function lift(self)
        engine.set_model(rawget(self, "_id"), "*1")
    end
    function thing(self) end
"#;

#[test]
fn a_script_finds_a_named_place() {
    let (game, _) = attaching_game("");
    game.exec(
        r#"
        local origin, angles = engine.place("door")
        engine.print(origin.x .. " " .. angles.y)
        engine.print(tostring(engine.place("window")))
        "#,
    )
    .expect("runs");
    assert_eq!(prints(&game), ["5 90", "nil"]);
}

#[test]
fn a_script_finds_a_place_on_a_model_and_holds_things_to_it() {
    let (game, simulation) = attaching_game(CLASSES);
    game.spawn_map(&[entity(&[("classname", "lift"), ("name", "up")])]);
    let lift = last(&simulation);
    let id = lift.to_bits();
    game.exec(&format!(
        r#"
        engine.print(engine.attach_point({id}, "spout").x)
        engine.print(tostring(engine.attach_point({id}, "nothing")))
        "#
    ))
    .expect("runs");
    assert_eq!(prints(&game), ["7", "nil"]);
}

/// A level's own `attach_to` joins up once everything's spawned, whichever came first.
#[test]
fn a_level_holds_one_thing_to_another_whatever_order_they_spawn_in() {
    let (game, simulation) = attaching_game(CLASSES);
    game.spawn_map(&[
        entity(&[
            ("classname", "thing"),
            ("attach_to", "up"),
            ("attach", "spout"),
        ]),
        entity(&[("classname", "lift"), ("name", "up")]),
    ]);
    let ids = simulation.borrow().ids();
    // The world comes first.
    let (thing, lift) = (ids[ids.len() - 2], ids[ids.len() - 1]);
    simulation
        .borrow_mut()
        .entity_mut(lift)
        .expect("there")
        .origin = Vec3::new(0.0, 0.0, 30.0);
    simulation.borrow_mut().end_tick(0.01);
    let held = simulation.borrow().entity(thing).expect("there").origin;
    assert_eq!(held, Vec3::new(7.0, 0.0, 30.0));
}

/// A mistake in a level says so on the console, and the rest still spawns.
#[test]
fn attaching_to_what_is_not_there_says_so() {
    let (game, _) = attaching_game(CLASSES);
    game.spawn_map(&[entity(&[("classname", "thing"), ("attach_to", "nobody")])]);
    let said = prints(&game);
    assert!(said.iter().any(|line| line.contains("nobody")), "{said:?}");
}

#[test]
fn a_script_starts_an_effect_with_its_own_settings_and_stops_it() {
    let (game, _simulation) = game("");
    game.exec(
        r#"
        local first = engine.effect("smoke", vector.create(1, 2, 3))
        local second = engine.effect("smoke", vector.create(4, 5, 6), nil, {
            color = { 1, 0.5, 0.25 },
            size = 2,
            strength = 3,
        })
        engine.effect_stop(second, 1.5)
        print(first ~= second and "different" or "same")
        "#,
    )
    .expect("runs");
    let outbox = game.take_outbox();
    assert_eq!(outbox.effects.len(), 3);
    let engine_core::entity::Effect::Named {
        handle: first,
        settings,
        ..
    } = outbox.effects[0]
    else {
        panic!("{:?}", outbox.effects[0]);
    };
    assert_eq!(settings, [1.0; 5], "with nothing said");
    let engine_core::entity::Effect::Named {
        handle: second,
        settings,
        ..
    } = outbox.effects[1]
    else {
        panic!("{:?}", outbox.effects[1]);
    };
    assert_eq!(settings, [1.0, 0.5, 0.25, 2.0, 3.0]);
    assert_ne!(first, second);
    assert_ne!(first, 0);
    assert_eq!(
        outbox.effects[2],
        engine_core::entity::Effect::NamedStops {
            handle: second,
            seconds: 1.5
        }
    );
}
