//! Every shader the engine has, and surface code, which is a shader's own part in
//! how a surface is drawn (proposal 0107).
//!
//! A shader is a piece of WGSL that replaces some of the hooks below. What it
//! leaves alone goes on working, so a short one can change one thing about an
//! ordinary lit surface, and one that replaces `shader_whole` can do anything.
//!
//! Shaders are compiled ahead of time, into the content's `compiled/shaders`, and that's
//! all a finished game reads (proposal 0133). With the `compiler` feature, which is for
//! working on things, what isn't compiled is compiled when it's asked for. This needs no
//! GPU, so tools can check shaders.

#[cfg(feature = "compiler")]
mod compiler;

use std::sync::OnceLock;

#[cfg(feature = "compiler")]
pub use compiler::{check_post_code, check_surface_code, compile, post_source, surface_source};

/// Changes when any of the engine's own shader code does, so that what was compiled from
/// older code is never taken for it.
const SHADERS_ID: &str = env!("ENGINE_SHADERS_ID");

/// What finds a file in a game's content.
type Reader = Box<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>;

/// How compiled shaders are found. A renderer makes its first pipelines as it starts, so this
/// is set once for the program ahead of that.
static COMPILED: OnceLock<Reader> = OnceLock::new();

/// Says how to read compiled shaders from a game's content, once, before a renderer starts.
pub fn read_compiled_with(reader: impl Fn(&str) -> Option<Vec<u8>> + Send + Sync + 'static) {
    let _first = COMPILED.set(Box::new(reader));
}

/// One compiled shader module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Module<'a> {
    /// The canvas.
    Canvas,
    /// Models.
    Alias,
    /// Sprites and particles.
    Sprite,
    /// Moving lights' shadows.
    Shadow,
    /// Edge smoothing.
    Smaa,
    /// What moves meshes.
    Deform,
    /// A surface with this surface code.
    Surface(&'a str),
    /// A surface with nothing unusual about it.
    PlainSurface,
    /// The final pass with this post code, reading depth as it was drawn if it has samples.
    Post {
        /// The post code.
        code: &'a str,
        /// Whether the scene has samples.
        samples: bool,
    },
}

impl Module<'_> {
    /// Where its compiled form is, in a game's content.
    #[must_use]
    pub fn path(&self) -> String {
        let name = match self {
            Self::Canvas => "canvas".to_owned(),
            Self::Alias => "alias".to_owned(),
            Self::Sprite => "sprite".to_owned(),
            Self::Shadow => "shadow".to_owned(),
            Self::Smaa => "smaa".to_owned(),
            Self::Deform => "deform".to_owned(),
            Self::PlainSurface => "plain".to_owned(),
            Self::Surface(code) => format!("surface_{:016x}", code_id(code)),
            Self::Post { code, samples } => {
                format!(
                    "post{}_{:016x}",
                    if *samples { "_samples" } else { "" },
                    code_id(code)
                )
            }
        };
        format!("compiled/shaders/{name}.spv")
    }
}

/// A number for `code`: FNV-1a over its bytes.
fn code_id(code: &str) -> u64 {
    code.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// What a SPIR-V module starts with.
const SPIRV_MAGIC: u32 = 0x0723_0203;

/// `words` as a file: the number of the shader code it came from, then the SPIR-V. The number
/// is here and not in its path, so that a file keeps its file id when shader code changes.
#[must_use]
pub fn compiled_file(words: &[u32]) -> Vec<u8> {
    let mut bytes = SHADERS_ID.as_bytes().to_vec();
    bytes.extend(words.iter().flat_map(|word| word.to_le_bytes()));
    bytes
}

/// The SPIR-V in `bytes`, if they're a compiled file of today's shader code.
fn words_of(bytes: &[u8]) -> Option<Vec<u32>> {
    let spirv = bytes.strip_prefix(SHADERS_ID.as_bytes())?;
    if spirv.len() % 4 != 0 {
        return None;
    }
    let words: Vec<u32> = spirv
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect();
    (words.first() == Some(&SPIRV_MAGIC)).then_some(words)
}

/// The SPIR-V of `module`: what's compiled, and otherwise, with the compiler, what comes of
/// compiling it now.
///
/// # Errors
///
/// Returns why it can't be had.
pub fn spirv(module: &Module<'_>) -> Result<Vec<u32>, String> {
    let path = module.path();
    let found = COMPILED.get().and_then(|read| read(&path));
    if let Some(words) = found.as_deref().and_then(words_of) {
        return Ok(words);
    }
    #[cfg(feature = "compiler")]
    {
        compile(module)
    }
    #[cfg(not(feature = "compiler"))]
    {
        Err(format!("{path} isn't there. A game needs its common.pkt"))
    }
}

/// Every module a game could want, for `surfaces` and `posts`, which are surface code and
/// post code. What's built in is always among them.
#[must_use]
pub fn all_modules<'a>(surfaces: &[&'a str], posts: &[&'a str]) -> Vec<Module<'a>> {
    let mut modules = vec![
        Module::Canvas,
        Module::Alias,
        Module::Sprite,
        Module::Shadow,
        Module::Smaa,
        Module::Deform,
        Module::PlainSurface,
        Module::Surface(""),
        Module::Surface(BROKEN),
    ];
    modules.extend(BUILT_IN.iter().map(|(_, code)| Module::Surface(code)));
    modules.extend(surfaces.iter().map(|code| Module::Surface(code)));
    for code in std::iter::once(&"").chain(posts) {
        modules.push(Module::Post {
            code,
            samples: false,
        });
        modules.push(Module::Post {
            code,
            samples: true,
        });
    }
    modules
}

/// What shows when a shader is missing or broken: green, so it can't be missed.
pub const BROKEN: &str = "fn shader_whole(in: VertexOut) -> vec4<f32> {
    return vec4<f32>(scene_color(vec3<f32>(0.0, 1.0, 0.0)), 1.0);
}
";

/// The engine's own shaders, for pictures with no material. A container can hold
/// its own under the same names.
pub const BUILT_IN: [(&str, &str); 4] = [
    ("classic", include_str!("shaders/surfaces/classic.wgsl")),
    (
        "classic_cutout",
        include_str!("shaders/surfaces/classic_cutout.wgsl"),
    ),
    (
        "classic_liquid",
        include_str!("shaders/surfaces/classic_liquid.wgsl"),
    ),
    (
        "classic_sky",
        include_str!("shaders/surfaces/classic_sky.wgsl"),
    ),
];

/// The names of the engine's own shaders.
#[must_use]
pub fn built_in_shaders() -> [&'static str; 4] {
    BUILT_IN.map(|(name, _)| name)
}

/// Whether a surface with `code` can be drawn after the depth pass, which draws
/// depth without it. Moving vertices or making holes changes depth.
pub fn safe_after_depth(code: &str) -> bool {
    !code.contains("fn shader_vertex") && !code.contains("discard")
}
