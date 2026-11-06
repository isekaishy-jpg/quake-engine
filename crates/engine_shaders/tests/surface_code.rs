//! Surface code, and that the engine's own shaders are sound.

use engine_shaders::{
    ALIAS_SHADER, BUILT_IN, CANVAS_SHADER, SHADOW_SHADER, SMAA_SHADER, SPRITE_SHADER,
    built_in_shaders, check_module, check_post_code, check_surface_code, post_source,
    safe_after_depth, surface_source,
};

#[test]
fn nothing_at_all_is_sound() {
    assert_eq!(check_surface_code(""), Ok(()));
    assert_eq!(check_post_code(""), Ok(()));
}

#[test]
fn nonsense_says_why() {
    assert!(check_surface_code("fn shader_whole(").is_err());
    assert!(check_post_code("fn shader_post(").is_err());
    let wrong = "fn shader_uv(uv: vec2<f32>, world: vec3<f32>) -> vec2<f32> { return world; }";
    assert!(check_surface_code(wrong).is_err(), "the wrong type");
}

#[test]
fn the_engines_own_surfaces_are_sound() {
    assert_eq!(built_in_shaders().len(), BUILT_IN.len());
    for (name, code) in BUILT_IN {
        assert_eq!(check_surface_code(code), Ok(()), "{name}");
    }
}

#[test]
fn the_other_shaders_are_sound() {
    for source in [
        ALIAS_SHADER,
        CANVAS_SHADER,
        SHADOW_SHADER,
        SMAA_SHADER,
        SPRITE_SHADER,
    ] {
        assert_eq!(check_module(source), Ok(()));
    }
}

#[test]
fn a_hook_thats_written_isnt_stood_in_for() {
    let code = "fn shader_uv(uv: vec2<f32>, world: vec3<f32>) -> vec2<f32> { return uv * 2.0; }";
    let source = surface_source(code);
    assert_eq!(source.matches("fn shader_uv").count(), 1);
    assert_eq!(source.matches("fn shader_vertex").count(), 1);
    assert_eq!(post_source("").matches("fn shader_post").count(), 1);
}

#[test]
fn what_changes_depth_cant_follow_the_depth_pass() {
    assert!(safe_after_depth(""));
    assert!(!safe_after_depth(
        "fn shader_vertex(world: vec3<f32>) -> vec3<f32> { return world; }"
    ));
    assert!(!safe_after_depth(
        "fn shader_whole(in: VertexOut) -> vec4<f32> { discard; }"
    ));
}
