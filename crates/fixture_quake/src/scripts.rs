//! The game's logic: QuakeC, ported to Luau (proposal 0027).

/// Every script, in load order.
pub const SCRIPTS: &[(&str, &str)] = &[
    ("subs", include_str!("../scripts/subs.luau")),
    ("doors", include_str!("../scripts/doors.luau")),
    ("plats", include_str!("../scripts/plats.luau")),
    ("buttons", include_str!("../scripts/buttons.luau")),
    ("triggers", include_str!("../scripts/triggers.luau")),
    ("misc", include_str!("../scripts/misc.luau")),
    ("world", include_str!("../scripts/world.luau")),
    ("client", include_str!("../scripts/client.luau")),
];
