//! A level as a game reads it, and a content folder as one pack (proposal 0106).

use std::sync::Arc;
use std::time::SystemTime;

use engine_assets::FileSystem;
use engine_core::math::Vec3;
use engine_format::Document;
use engine_map::{
    EffectDef, EffectPoint, EmitterShape, Entity, LandPicture, Map, ParticleKind, ShaderCode,
    SlotImage, Texture, mip_chain,
};

use crate::bake::map_hash;
use crate::build::{TextureSource, compile, compile_known};
use crate::compiled::read_geometry;
use crate::effect_read::read_effect;
use crate::error::CompileError;
use crate::land_pictures::{plain_pictures, scaled};
use crate::material::{MaterialLibrary, document_of};
use crate::mesh::surface_points;
use crate::source::{WorkingMap, read_map};

/// A level's textures while it loads: compiled for the GPU, and in pixels for
/// what's read here. The first thing that goes wrong is kept for afterwards.
struct PlayTextures<'a> {
    /// Where they come from.
    library: MaterialLibrary<'a>,
    /// What went wrong.
    failure: Option<CompileError>,
}

impl PlayTextures<'_> {
    /// `result`'s texture, or nothing, with the error kept.
    fn kept(&mut self, result: Result<Texture, CompileError>) -> Texture {
        result.unwrap_or_else(|error| {
            self.failure = Some(error);
            Texture::default()
        })
    }
}

impl TextureSource for PlayTextures<'_> {
    fn texture(&mut self, material: &str) -> Texture {
        let result = self.library.load(material);
        self.kept(result)
    }

    fn pixels(&mut self, material: &str) -> Texture {
        let result = self.library.load_pixels(material);
        self.kept(result)
    }

    fn land_pictures(&mut self, materials: &[String]) -> Vec<LandPicture> {
        if let Some(pictures) = self.library.packed_land(materials) {
            return pictures;
        }
        let textures: Vec<Texture> = materials.iter().map(|name| self.pixels(name)).collect();
        plain_pictures(materials, &textures)
    }
}

/// Every effect `filesystem` holds.
fn effects_in(filesystem: &FileSystem) -> Result<Vec<Arc<EffectDef>>, CompileError> {
    let mut out = Vec::new();
    for path in filesystem.list("effects/") {
        if let Some(name) = path
            .strip_prefix("effects/")
            .and_then(|file| file.strip_suffix(".eeffect"))
        {
            let bytes = filesystem
                .read(&path)
                .map_err(|error| CompileError::Bad(error.to_string()))?;
            let read = document_of(&bytes).and_then(|document| read_effect(name, &document));
            let effect = read.map_err(|error| CompileError::Bad(format!("{path}: {error}")))?;
            out.push(Arc::new(effect));
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// The widest a picture of a particle gets.
const WIDEST_PICTURE: u32 = 512;

/// A level's effects, with what they show.
#[derive(Clone, Debug)]
pub struct EffectExtras {
    /// The effects.
    pub effects: Vec<Arc<EffectDef>>,
    /// Their pictures.
    pub pictures: Vec<SlotImage>,
    /// Their shaders.
    pub shaders: Vec<Arc<ShaderCode>>,
    /// Their shaders for how compute moves them.
    pub motions: Vec<Arc<ShaderCode>>,
    /// The meshes they're made of that the level hasn't got, as it was read before they were said.
    pub missing_meshes: Vec<String>,
}

/// What `level` has that effects go by: the names of its fields and its moving meshes.
fn names_in(level: &Map) -> (Vec<String>, Vec<String>) {
    let fields = level.fields.iter().map(|field| field.name.clone());
    let meshes = level.moving_meshes.iter().map(|mesh| mesh.name.clone());
    (fields.collect(), meshes.collect())
}

/// Every effect `filesystem` holds as `level` has them, read afresh.
///
/// # Errors
///
/// Returns an error if one is wrong, or something it shows is.
pub fn read_effects(filesystem: &FileSystem, level: &Map) -> Result<EffectExtras, CompileError> {
    let mut library = MaterialLibrary::for_play(filesystem);
    let (fields, meshes) = names_in(level);
    effect_extras(&mut library, effects_in(filesystem)?, (&fields, &meshes))
}

/// The same with `edited` in place of the one of its name, for an effect that's being changed.
///
/// # Errors
///
/// Returns an error if one is wrong, or something it shows is.
pub fn read_effects_with(
    filesystem: &FileSystem,
    level: &Map,
    edited: EffectDef,
) -> Result<EffectExtras, CompileError> {
    let mut library = MaterialLibrary::for_play(filesystem);
    let mut effects = effects_in(filesystem)?;
    effects.retain(|effect| effect.name != edited.name);
    effects.push(Arc::new(edited));
    let (fields, meshes) = names_in(level);
    effect_extras(&mut library, effects, (&fields, &meshes))
}

/// Keeps the meshes `filesystem`'s effects are made of, as things in a level can be. It comes after
/// what a level's bake goes by, so that changing an effect doesn't make a level's light old.
///
/// # Errors
///
/// Returns an error if an effect is wrong.
pub fn keep_effect_meshes(
    filesystem: &FileSystem,
    library: &mut MaterialLibrary<'_>,
    working: &mut WorkingMap,
) -> Result<(), CompileError> {
    for effect in effects_in(filesystem)? {
        let wanted = effect
            .emitters
            .iter()
            .filter(|e| e.kind == ParticleKind::Mesh);
        for name in wanted.filter_map(|emitter| emitter.mesh.as_ref()) {
            if working.moving.iter().any(|(known, _)| known == name) {
                continue;
            }
            if let Ok(loaded) = library.mesh(name) {
                working.moving.push((name.clone(), loaded.mesh));
            }
        }
    }
    Ok(())
}

/// Every file effects come from and when it changed, which says when to read them again. There's
/// nothing to say of a file what holds it can't date, so what ships has an empty list.
#[must_use]
pub fn effect_files(filesystem: &FileSystem) -> Vec<(String, SystemTime)> {
    let mut paths = filesystem.list("effects/");
    paths.retain(|path| path.ends_with(".eeffect"));
    let effects = paths.clone();
    for path in effects {
        let bytes = filesystem.read(&path).unwrap_or_default();
        let Ok(effect) = document_of(&bytes).and_then(|document| read_effect("", &document)) else {
            continue;
        };
        for emitter in &effect.emitters {
            paths.extend(emitter.image.clone());
            for name in emitter.shader.iter().chain(&emitter.motion) {
                paths.push(format!("shaders/{name}.wgsl"));
                paths.push(format!("shaders/{name}.egraph"));
            }
        }
    }
    paths.sort();
    paths.dedup();
    let dated = paths.into_iter().filter_map(|path| {
        let changed = filesystem.changed(&path)?;
        Some((path, changed))
    });
    dated.collect()
}

/// `effects`, with their emitters told where their pictures and shaders are, and those pictures
/// and shaders, each once. Pictures come to one size, for one array.
fn effect_extras(
    library: &mut MaterialLibrary<'_>,
    effects: Vec<Arc<EffectDef>>,
    (fields, meshes): (&[String], &[String]),
) -> Result<EffectExtras, CompileError> {
    let mut surfaces = 0;
    let mut missing_meshes: Vec<String> = Vec::new();
    let mut paths: Vec<String> = Vec::new();
    let mut shaders: Vec<String> = Vec::new();
    let mut motions: Vec<String> = Vec::new();
    let mut out = Vec::with_capacity(effects.len());
    for effect in effects {
        let mut effect = EffectDef::clone(&effect);
        for emitter in &mut effect.emitters {
            if let Some(path) = &emitter.image {
                let at = paths.iter().position(|known| known == path);
                let at = at.unwrap_or_else(|| {
                    paths.push(path.clone());
                    paths.len() - 1
                });
                emitter.picture = u32::try_from(at + 1).unwrap_or(0);
            }
            if let Some(name) = &emitter.shader {
                let at = shaders.iter().position(|known| known == name);
                let at = at.unwrap_or_else(|| {
                    shaders.push(name.clone());
                    shaders.len() - 1
                });
                emitter.shader_number = u32::try_from(at + 1).unwrap_or(0);
            }
            if let Some(name) = &emitter.motion {
                let at = motions.iter().position(|known| known == name);
                let at = at.unwrap_or_else(|| {
                    motions.push(name.clone());
                    motions.len() - 1
                });
                emitter.motion_number = u32::try_from(at + 1).unwrap_or(0);
            }
            if let Some(name) = emitter
                .mesh
                .as_ref()
                .filter(|_| emitter.kind == ParticleKind::Mesh)
            {
                let at = meshes.iter().position(|known| known == name);
                emitter.mesh_number = at.map_or(0, |at| u32::try_from(at + 1).unwrap_or(0));
                if at.is_none() && !missing_meshes.contains(name) {
                    missing_meshes.push(name.clone());
                }
            }
            if emitter.shape == EmitterShape::Surface {
                let name = emitter.surface_of.as_ref().or(emitter.mesh.as_ref());
                // An effect goes with every level, so a mesh that's gone is for `check_effects`.
                let loaded = name.and_then(|name| library.mesh(name).ok());
                emitter.surface =
                    loaded.map_or_else(Vec::new, |loaded| surface_points(&loaded.mesh));
                emitter.surface_first = surfaces;
                surfaces += u32::try_from(emitter.surface.len()).unwrap_or(0);
            }
            if let Some(name) = &emitter.field {
                let at = fields.iter().position(|known| known == name);
                emitter.field_number = at.map_or(0, |at| u32::try_from(at + 1).unwrap_or(0));
            }
        }
        out.push(Arc::new(effect));
    }
    let mut read = Vec::with_capacity(paths.len());
    for path in &paths {
        read.push(library.image_at(path)?);
    }
    let widest = read
        .iter()
        .filter_map(|image| image.first())
        .map(|top| top.width.max(top.height));
    let side = widest
        .max()
        .unwrap_or(1)
        .next_power_of_two()
        .min(WIDEST_PICTURE);
    let pictures = read
        .iter()
        .filter_map(|image| image.first())
        .map(|top| Arc::new(mip_chain(side, side, scaled(top, side))))
        .collect();
    let shaders = shaders.iter().map(|name| library.shader(name)).collect();
    let motions = motions.iter().map(|name| library.shader(name)).collect();
    Ok(EffectExtras {
        effects: out,
        pictures,
        shaders,
        motions,
        missing_meshes,
    })
}

/// A level ready to play.
#[derive(Debug)]
pub struct PlayMap {
    /// The map, without its bake.
    pub map: Map,
    /// The hash a bake has to have.
    pub hash: u64,
    /// The pictures it read as pixels, which a pack has to keep.
    pub pictures: Vec<String>,
}

/// The number of the model of the entity called `name`.
fn model_named(map: &Map, name: &str) -> Option<u32> {
    let named = |entity: &&Entity| {
        entity.get("name") == Some(name) || entity.get("targetname") == Some(name)
    };
    let model = map.entities.iter().find(named)?.get("model")?;
    model.strip_prefix('*')?.parse().ok()
}

/// Reads the map called `name` from `document` as a game does: with compiled
/// pictures, and the geometry an earlier compile kept, where they're current.
///
/// # Errors
///
/// Returns an error if the map or something it uses is broken.
pub fn play_map(
    filesystem: &FileSystem,
    name: &str,
    document: &Document,
) -> Result<PlayMap, CompileError> {
    let mut working = read_map(document)?;
    let mut library = MaterialLibrary::for_play(filesystem);
    let used = library.prepare(&mut working)?;
    let hash = map_hash(document, &used);
    keep_effect_meshes(filesystem, &mut library, &mut working)?;
    let mut textures = PlayTextures {
        library,
        failure: None,
    };
    let kept = filesystem.read(&format!("maps/{name}.egeo")).ok();
    let map = match kept.and_then(|bytes| read_geometry(&bytes, hash)) {
        Some(geometry) => compile_known(&working, &mut textures, geometry)?,
        None => compile(&working, &mut textures)?,
    };
    if let Some(error) = textures.failure {
        return Err(error);
    }
    let mut map = map;
    let world = working.entities.iter().find(|e| e.kind == "worldspawn");
    let post = world.and_then(|world| world.text("post_shader"));
    map.post_shader = post.map(|name| textures.library.shader(name));
    map.effects = effects_in(filesystem)?;
    let (fields, meshes) = names_in(&map);
    let effects = std::mem::take(&mut map.effects);
    let extras = effect_extras(&mut textures.library, effects, (&fields, &meshes))?;
    map.effects = extras.effects;
    map.effect_pictures = extras.pictures;
    map.effect_shaders = extras.shaders;
    map.effect_motions = extras.motions;
    for entity in working.entities.iter().filter(|e| e.kind == "effect") {
        let name = entity.text("effect").unwrap_or_default();
        let [x, y, z] = entity.point("origin").unwrap_or([0.0; 3]);
        #[expect(clippy::cast_possible_truncation, reason = "maps store f32")]
        let origin = Vec3::new(x as f32, y as f32, z as f32);
        let model = entity
            .text("attach_to")
            .and_then(|name| model_named(&map, name));
        map.effect_points.push(EffectPoint {
            effect: name.to_owned(),
            origin,
            model,
        });
    }
    let pictures = textures.library.pixel_pictures();
    Ok(PlayMap {
        map,
        hash,
        pictures,
    })
}
