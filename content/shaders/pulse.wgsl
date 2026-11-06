// Makes a surface glow in its own colour, and fade, over and over.
// Numbers: how bright at the top, and how fast.
fn shader_surface(surface: Surface) -> Surface {
    var out = surface;
    let beat = 0.5 + 0.5 * sin(shader_time() * shader_param(1));
    out.emissive = surface.base.rgb * beat * shader_param(0);
    return out;
}
