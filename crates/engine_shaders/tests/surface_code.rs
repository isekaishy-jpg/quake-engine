//! Surface code, and that the engine's own shaders are sound.

use engine_shaders::{
    BUILT_IN, all_modules, built_in_shaders, check_post_code, check_surface_code, compile,
    post_source, safe_after_depth, surface_source,
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
fn every_module_compiles() {
    let field = "fn shader_field(at: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    return field_read(at);
}
";
    let particle = "fn shader_particle(look: ParticleLook) -> vec4<f32> {
    return look.color * shader_noise(look.uv);
}
";
    let motion = "fn shader_motion(particle: SimParticle, seconds: f32) -> SimParticle {
    var out = particle;
    out.moving = out.moving + field_at(out.place.xy) * seconds;
    return out;
}
";
    for module in all_modules(&[], &[], &[field], (&[particle], &[motion])) {
        assert!(compile(&module).is_ok(), "{module:?}");
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
