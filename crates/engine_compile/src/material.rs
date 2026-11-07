//! Reading materials: `.emat` documents and the images they name.

use std::collections::HashMap;
use std::sync::Arc;

use engine_assets::FileSystem;
use engine_format::{Block, Document, Value, from_binary, from_text};
use engine_map::{
    AlphaMode, LandPicture, MATERIAL_SLOTS, MAX_LAYERS, Material, MaterialLayer, MetalRough,
    MipLevel, PAINT_MASK, SHADER_PARAMS, ShaderCode, Shading, SlotImage, TerrainData, Texture,
    TextureKind, mip_chain,
};

use crate::convex::fit_convex;
use crate::error::CompileError;
use crate::land_pictures::{SurfaceMaps, packed_pictures, surface_pixels};
use crate::mesh::{LoadedMesh, read_mesh, read_occlusion, resolve_meshes};
use crate::shader_graph::{graph_code, read_graph};
use crate::source::{WorkingMap, read_map};
use crate::terrain::{read_terrain_data, write_terrain};
use crate::texture::placeholder_texture;
use crate::texture_pack::{bytes_hash, packed_path, picture_roles, read_packed};
use crate::tiles::{check_tiles, merge_tile};

/// Decodes a PNG into RGBA bytes.
///
/// # Errors
///
/// Returns an error if `bytes` aren't a PNG this can read.
pub fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), CompileError> {
    let bad = |error: &dyn std::fmt::Display| CompileError::Bad(format!("bad PNG: {error}"));
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|error| bad(&error))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| CompileError::Bad("a PNG is too big".to_owned()))?;
    let mut pixels = vec![0; size];
    let info = reader
        .next_frame(&mut pixels)
        .map_err(|error| bad(&error))?;
    pixels.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels,
        png::ColorType::Rgb => pixels
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|[red, green, blue]| [*red, *green, *blue, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|[grey, alpha]| [*grey, *grey, *grey, *alpha])
            .collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|p| [*p, *p, *p, 255]).collect(),
        png::ColorType::Indexed => {
            return Err(CompileError::Bad(
                "an indexed PNG wasn't expanded".to_owned(),
            ));
        }
    };
    Ok((info.width, info.height, rgba))
}

/// A field's number, or `default`.
fn number(block: &Block, key: &str, default: f32) -> f32 {
    #[expect(clippy::cast_possible_truncation, reason = "materials store f32")]
    block
        .get(key)
        .and_then(Value::number)
        .map_or(default, |value| value as f32)
}

/// A field's numbers, or `default`.
fn numbers<const N: usize>(block: &Block, key: &str, default: [f32; N]) -> [f32; N] {
    let mut out = default;
    if let Some(values) = block.get(key).and_then(Value::numbers)
        && values.len() == N
    {
        for (slot, value) in out.iter_mut().zip(values) {
            #[expect(clippy::cast_possible_truncation, reason = "materials store f32")]
            let value = value as f32;
            *slot = value;
        }
    }
    out
}

/// The numbers a material gives its shader: as many as it has, and 0 for the
/// rest.
fn shader_params(block: &Block) -> [f32; SHADER_PARAMS] {
    let mut out = [0.0; SHADER_PARAMS];
    let Some(value) = block.get("shader_params") else {
        return out;
    };
    let one = value.number().map(|one| vec![one]);
    let many = value.numbers().map(|many| many.to_vec());
    for (slot, number) in out.iter_mut().zip(many.or(one).unwrap_or_default()) {
        #[expect(clippy::cast_possible_truncation, reason = "materials store f32")]
        let number = number as f32;
        *slot = number;
    }
    out
}

/// Mixes a material's image-masked layers into its base, so they cost nothing
/// when drawn, and frees their images. Painted layers stay, since they differ
/// from one surface to the next.
fn flatten_layers(material: &mut Material, mips: Vec<MipLevel>) -> Vec<MipLevel> {
    let Some(base) = mips.first() else {
        return mips;
    };
    let (width, height) = (base.width as usize, base.height as usize);
    let mut rgba = base.rgba.clone();
    let slot_image =
        |slots: &[SlotImage], slot: u8| slots.get(usize::from(slot).wrapping_sub(1)).cloned();
    let at_size = |image: &MipLevel, x: usize, y: usize| {
        let (u, v) = (
            x * image.width as usize / width,
            y * image.height as usize / height,
        );
        (v * image.width as usize + u) * 4
    };
    let mut kept = Vec::new();
    for layer in std::mem::take(&mut material.layers) {
        let images = (
            slot_image(&material.slots, layer.base),
            slot_image(&material.slots, layer.mask),
        );
        // A layer with bumps and shine of its own has more than a colour to give, so it stays.
        let (Some(color), Some(mask), None) = (images.0, images.1, layer.surface) else {
            kept.push(layer);
            continue;
        };
        let (Some(color), Some(mask)) = (color.first(), mask.first()) else {
            continue;
        };
        for y in 0..height {
            for x in 0..width {
                let at = (y * width + x) * 4;
                let share = u32::from(mask.rgba[at_size(mask, x, y)]);
                let from = at_size(color, x, y);
                for part in 0..3 {
                    let (old, new) = (
                        u32::from(rgba[at + part]),
                        u32::from(color.rgba[from + part]),
                    );
                    let mixed = (old * (255 - share) + new * share) / 255;
                    rgba[at + part] = u8::try_from(mixed).unwrap_or(u8::MAX);
                }
            }
        }
        for slot in [layer.base, layer.mask] {
            if let Some(image) = material.slots.get_mut(usize::from(slot).wrapping_sub(1)) {
                *image = Arc::new(vec![MipLevel {
                    width: 1,
                    height: 1,
                    rgba: vec![0; 4],
                    form: engine_map::PixelForm::Rgba,
                }]);
            }
        }
    }
    material.layers = kept;
    mip_chain(base.width, base.height, rgba)
}

/// How much of the world a material's repeat covers, if it says.
fn world_size_of(block: &Block) -> Option<[f32; 2]> {
    let size = numbers(block, "world_size", [0.0; 2]);
    (size[0] > 0.0 && size[1] > 0.0).then_some(size)
}

/// A material's hotspot areas.
#[expect(clippy::cast_possible_truncation, reason = "materials store f32")]
fn hotspots_of(block: &Block) -> Vec<[f32; 4]> {
    let numbers = block
        .get("hotspots")
        .and_then(Value::numbers)
        .unwrap_or_default();
    let (areas, _) = numbers.as_chunks::<4>();
    areas
        .iter()
        .map(|area| area.map(|part| part as f32))
        .collect()
}

/// A document from its bytes, in text or binary.
///
/// # Errors
///
/// Returns an error if they aren't one.
pub fn document_of(bytes: &[u8]) -> Result<Document, CompileError> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(from_text(text)?),
        Err(_) => Ok(from_binary(bytes)?),
    }
}

/// Loads materials by name, keeping each image once.
pub struct MaterialLibrary<'a> {
    /// Meshes read so far.
    meshes: HashMap<String, LoadedMesh>,
    /// Where files come from.
    filesystem: &'a FileSystem,
    /// Images already decoded, by path.
    images: HashMap<String, SlotImage>,
    /// Whether this is for playing, not baking.
    playing: bool,
    /// Whether the material being read can use compiled pictures. Layers are
    /// mixed into their base here, which takes pixels.
    compiled_pictures: bool,
}

impl std::fmt::Debug for MaterialLibrary<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialLibrary")
            .field("images", &self.images.len())
            .finish_non_exhaustive()
    }
}

/// How many parents a material can have above it.
const MAX_INHERITANCE: usize = 8;

/// Gives `document`'s surface what `parent`'s has and it doesn't. Its own layers,
/// if it has any, replace the parent's.
fn inherit(document: &mut Document, parent: &Document) {
    let surface = document
        .blocks
        .iter_mut()
        .find(|block| block.kind == "surface");
    let (Some(surface), Some(from)) = (surface, parent.blocks_of("surface").next()) else {
        return;
    };
    for (key, value) in &from.fields {
        if surface.get(key).is_none() {
            surface.fields.push((key.clone(), value.clone()));
        }
    }
    if surface.children.is_empty() {
        surface.children.clone_from(&from.children);
    }
}

/// Fills a material's slots as images are named.
struct SlotFiller<'a, 'b> {
    /// Where images come from.
    library: &'a mut MaterialLibrary<'b>,
    /// The material's name, for errors.
    name: &'a str,
    /// The images so far, after the base.
    slots: Vec<SlotImage>,
}

impl SlotFiller<'_, '_> {
    /// Takes a slot for `path`, reusing one already holding it.
    fn add(&mut self, path: &str) -> Result<u8, CompileError> {
        let image = self.library.image(path)?;
        self.keep(image)
    }

    /// Takes a slot for `image`, reusing one already holding it.
    fn keep(&mut self, image: SlotImage) -> Result<u8, CompileError> {
        let found = self
            .slots
            .iter()
            .position(|known| Arc::ptr_eq(known, &image));
        let index = if let Some(found) = found {
            found
        } else {
            self.slots.push(image);
            self.slots.len() - 1
        };
        if index + 2 > MATERIAL_SLOTS {
            return Err(CompileError::Bad(format!(
                "{} uses more than {MATERIAL_SLOTS} images",
                self.name
            )));
        }
        Ok(u8::try_from(index + 1).unwrap_or(u8::MAX))
    }

    /// The slot of a layer's own bumps and shine, if its block names any, at the size of the
    /// colour in `base`.
    fn layer_surface(&mut self, block: &Block, base: u8) -> Result<Option<u8>, CompileError> {
        let wanted = ["normal", "specular", "height"]
            .map(|key| block.get(key).and_then(Value::text).map(str::to_owned));
        if wanted.iter().all(Option::is_none) {
            return Ok(None);
        }
        let color = self
            .slots
            .get(usize::from(base).wrapping_sub(1))
            .map(Arc::clone)
            .ok_or_else(|| CompileError::Bad(format!("{}: a layer with no base", self.name)))?;
        let [normal, specular, height] = wanted;
        let read = |library: &mut MaterialLibrary<'_>, path: Option<String>| {
            path.map(|path| library.image(&path)).transpose()
        };
        let normal = read(self.library, normal)?;
        let specular = read(self.library, specular)?;
        let height = read(self.library, height)?;
        let top =
            |image: &Option<SlotImage>| image.as_ref().and_then(|image| image.first().cloned());
        let (normal, specular, height) = (top(&normal), top(&specular), top(&height));
        let Some(first) = color.first() else {
            return Ok(None);
        };
        let pixels = surface_pixels(
            first,
            SurfaceMaps {
                normal: normal.as_ref(),
                height: height.as_ref(),
                specular: specular.as_ref(),
                strength: number(block, "specular_strength", 1.0),
            },
        );
        self.keep(Arc::new(mip_chain(first.width, first.height, pixels)))
            .map(Some)
    }

    /// The slot of the image `key` names, if it names one.
    fn field(&mut self, block: &Block, key: &str) -> Result<Option<u8>, CompileError> {
        block
            .get(key)
            .and_then(Value::text)
            .map(|path| self.add(path))
            .transpose()
    }
}

impl<'a> MaterialLibrary<'a> {
    /// A library reading from `filesystem`.
    #[must_use]
    pub fn new(filesystem: &'a FileSystem) -> Self {
        Self {
            filesystem,
            images: HashMap::new(),
            meshes: HashMap::new(),
            playing: false,
            compiled_pictures: false,
        }
    }

    /// A library for playing: it reads pictures as they were compiled, where it
    /// can, since a game only hands them on to the GPU. A bake needs the pixels.
    #[must_use]
    pub fn for_play(filesystem: &'a FileSystem) -> Self {
        Self {
            playing: true,
            ..Self::new(filesystem)
        }
    }

    /// The compiled form of the picture at `path`, if there's one and its source
    /// hasn't changed since. What ships has no source to compare.
    fn packed(&self, path: &str) -> Option<SlotImage> {
        let bytes = self.filesystem.read(&packed_path(path)).ok()?;
        let (hash, mips) = read_packed(&bytes)?;
        let source = self.filesystem.read(path).ok();
        let current = source.is_none_or(|source| bytes_hash(&source) == hash);
        current.then(|| Arc::new(mips))
    }

    /// The image at `path`, with all its mips.
    fn image(&mut self, path: &str) -> Result<SlotImage, CompileError> {
        if self.compiled_pictures {
            let key = packed_path(path);
            if let Some(image) = self.images.get(&key) {
                return Ok(Arc::clone(image));
            }
            if let Some(image) = self.packed(path) {
                self.images.insert(key, Arc::clone(&image));
                return Ok(image);
            }
        }
        if let Some(image) = self.images.get(path) {
            return Ok(Arc::clone(image));
        }
        let bytes = self
            .filesystem
            .read(path)
            .map_err(|error| CompileError::Bad(error.to_string()))?;
        let (width, height, rgba) = decode_png(&bytes)?;
        let image = Arc::new(mip_chain(width, height, rgba));
        self.images.insert(path.to_owned(), Arc::clone(&image));
        Ok(image)
    }

    /// Gets `working` ready to compile: merges its tiles into place, and places
    /// its meshes. Returns the text of everything that took, which a bake's hash
    /// has to cover along with the map's own.
    ///
    /// # Errors
    ///
    /// Returns an error if something can't be read.
    pub fn prepare(&mut self, working: &mut WorkingMap) -> Result<String, CompileError> {
        check_tiles(working)?;
        let mut read = String::new();
        for tile in working.tiles.clone() {
            let path = format!("maps/{}.etile", tile.file);
            let bytes = self
                .filesystem
                .read(&path)
                .map_err(|error| CompileError::Bad(format!("{path}: {error}")))?;
            let text = std::str::from_utf8(&bytes)
                .map_err(|error| CompileError::Bad(format!("{path}: {error}")))?;
            let part = read_map(&from_text(text)?)?;
            let size = working.tile_size;
            let offset = [f64::from(tile.x) * size, f64::from(tile.y) * size, 0.0];
            merge_tile(working, part, offset);
            read.push_str(text);
        }
        let mut names: Vec<String> = working.meshes.iter().map(|m| m.source.clone()).collect();
        names.extend(
            working
                .entities
                .iter()
                .filter_map(|e| e.mesh.as_ref().map(|m| m.source.clone())),
        );
        names.extend(working.moving_names());
        names.sort();
        names.dedup();
        for name in names {
            let path = format!("meshes/{name}.emesh");
            if let Ok(bytes) = self.filesystem.read(&path) {
                read.push_str(&String::from_utf8_lossy(&bytes));
            }
        }
        for sheet in &mut working.terrain {
            let path = format!("terrain/{}.eterrain", sheet.source);
            let bytes = self
                .filesystem
                .read(&path)
                .map_err(|error| CompileError::Bad(format!("{path}: {error}")))?;
            sheet.data = read_terrain_data(&bytes)?;
            // Light goes by the land's shape, so a bake lasts through paint.
            let shape = TerrainData {
                layers: Vec::new(),
                paint: Vec::new(),
                ..sheet.data.clone()
            };
            read.push_str(&String::from_utf8_lossy(&write_terrain(&shape)));
        }
        resolve_meshes(working, &mut |name| self.mesh(name))?;
        Ok(read)
    }

    /// The mesh called `name`, with its sidecar's volume if it has one.
    ///
    /// # Errors
    ///
    /// Returns an error if it can't be read.
    pub fn mesh(&mut self, name: &str) -> Result<LoadedMesh, CompileError> {
        if let Some(loaded) = self.meshes.get(name) {
            return Ok(loaded.clone());
        }
        let path = format!("meshes/{name}.emesh");
        let bytes = self
            .filesystem
            .read(&path)
            .map_err(|error| CompileError::Bad(format!("{path}: {error}")))?;
        let mut mesh = read_mesh(&document_of(&bytes)?)?;
        if mesh.hulls.is_empty() && mesh.convex.is_empty() {
            mesh.convex = fit_convex(&mesh);
        }
        let occluders = self
            .filesystem
            .read(&format!("meshes/{name}.eocc"))
            .ok()
            .and_then(|bytes| document_of(&bytes).ok())
            .map_or_else(Vec::new, |document| read_occlusion(&document));
        let loaded = LoadedMesh {
            mesh: Arc::new(mesh),
            occluders,
        };
        self.meshes.insert(name.to_owned(), loaded.clone());
        Ok(loaded)
    }

    /// A hash of what `material`'s pictures for land are made from.
    pub(crate) fn land_hash(&self, material: &str) -> Option<u64> {
        let document = self.document_named(material, 0).ok()??;
        let mut bytes = self
            .filesystem
            .read(&format!("materials/{material}.emat"))
            .ok()?;
        for (picture, _) in picture_roles(&document) {
            bytes.extend(self.filesystem.read(&picture).ok()?);
        }
        Some(bytes_hash(&bytes))
    }

    /// What `materials` are to land, as they were compiled, if that's there and up to date.
    pub(crate) fn packed_land(&mut self, materials: &[String]) -> Option<Vec<LandPicture>> {
        let filesystem = self.filesystem;
        let hashes: Vec<Option<u64>> = materials.iter().map(|m| self.land_hash(m)).collect();
        let mut asked = hashes.into_iter();
        let mut glosses = Vec::with_capacity(materials.len());
        for material in materials {
            let texture = self.load(material).ok()?;
            glosses.push(texture.material.map_or(32.0, |material| material.gloss));
        }
        let mut gloss = glosses.into_iter();
        packed_pictures(
            filesystem,
            materials,
            &mut |_| asked.next().flatten(),
            &mut |_| gloss.next().unwrap_or(32.0),
        )
    }

    /// The pictures read so far as plain pixels.
    #[must_use]
    pub fn pixel_pictures(&self) -> Vec<String> {
        self.images
            .keys()
            .filter(|path| !path.starts_with("compiled/"))
            .cloned()
            .collect()
    }

    /// The image at `path`, for what isn't a material, such as a light's
    /// cookie.
    ///
    /// # Errors
    ///
    /// Returns an error if it can't be read.
    pub fn image_at(&mut self, path: &str) -> Result<SlotImage, CompileError> {
        self.compiled_pictures = false;
        self.image(path)
    }

    /// Reads a material document into a texture.
    ///
    /// # Errors
    ///
    /// Returns an error if it makes no sense or an image can't be read.
    pub fn read(&mut self, name: &str, document: &Document) -> Result<Texture, CompileError> {
        self.read_as(name, document, self.playing)
    }

    /// Reads a material document into a texture, with pictures as they were
    /// compiled if `compiled` says so.
    fn read_as(
        &mut self,
        name: &str,
        document: &Document,
        compiled: bool,
    ) -> Result<Texture, CompileError> {
        if document.kind != "material" {
            return Err(CompileError::Bad(format!("{name} isn't a material")));
        }
        let block = document
            .blocks_of("surface")
            .next()
            .ok_or_else(|| CompileError::Bad(format!("{name} has no surface")))?;
        let base = block
            .get("base")
            .and_then(Value::text)
            .ok_or_else(|| CompileError::Bad(format!("{name} has no base")))?;
        self.compiled_pictures =
            compiled && block.children.iter().all(|child| child.kind != "layer");
        let mips: Vec<MipLevel> = self.image(base)?.as_ref().clone();
        let mut filler = SlotFiller {
            library: self,
            name,
            slots: Vec::new(),
        };
        let mut material = Material {
            specular_strength: number(block, "specular_strength", 1.0),
            gloss: number(block, "gloss", 32.0),
            parallax: number(block, "parallax", 0.0),
            detail_scale: number(block, "detail_scale", 8.0),
            emissive_strength: number(block, "emissive_strength", 1.0),
            cutoff: number(block, "cutoff", 0.5),
            flow_strength: number(block, "flow_strength", 0.1),
            flow_speed: number(block, "flow_speed", 0.5),
            reflect: number(block, "reflect", 0.0),
            mega: block.get("mega").and_then(Value::flag) == Some(true),
            detail_fade: block.get("fade").and_then(Value::flag) == Some(true),
            hotspots: hotspots_of(block),
            world_size: world_size_of(block),
            scroll: numbers(block, "scroll", [0.0; 2]),
            flipbook: numbers(block, "flipbook", [0.0; 3]),
            triplanar: block
                .get("triplanar")
                .and_then(Value::flag)
                .unwrap_or(false),
            hex_tiling: block.get("hex").and_then(Value::flag).unwrap_or(false),
            decal: block.get("decal").and_then(Value::flag).unwrap_or(false),
            shading: match block.get("shading").and_then(Value::text) {
                None | Some("lit") => Shading::Lit,
                Some("cel") => Shading::Cel,
                Some("matcap") => Shading::Matcap,
                Some("unlit") => Shading::Unlit,
                Some(other) => {
                    return Err(CompileError::Bad(format!("{name}: shading {other}")));
                }
            },
            alpha: match block.get("alpha").and_then(Value::text) {
                None | Some("opaque") => AlphaMode::Opaque,
                Some("mask") => AlphaMode::Mask,
                Some("blend") => AlphaMode::Blend,
                Some(other) => return Err(CompileError::Bad(format!("{name}: alpha {other}"))),
            },
            ..Material::default()
        };
        let shader_name = block.get("shader").and_then(Value::text);
        material.shader = shader_name.map(|name| filler.library.shader(name));
        material.shader_params = shader_params(block);
        let metal_map = filler.field(block, "metal_rough")?;
        let says_metal = block.get("metallic").is_some() || block.get("roughness").is_some();
        if metal_map.is_some() || says_metal {
            // With a map these scale it, so 1 leaves it as it is.
            let (metallic, roughness) = if metal_map.is_some() {
                (1.0, 1.0)
            } else {
                (0.0, 0.5)
            };
            material.metal_rough = Some(MetalRough {
                metallic: number(block, "metallic", metallic),
                roughness: number(block, "roughness", roughness),
                map: metal_map,
            });
        }
        material.shader_images = [
            filler.field(block, "shader_image_a")?,
            filler.field(block, "shader_image_b")?,
        ];
        material.normal = filler.field(block, "normal")?;
        material.specular = filler.field(block, "specular")?;
        material.emissive = filler.field(block, "emissive")?;
        material.height = filler.field(block, "height")?;
        material.tint = filler.field(block, "tint")?;
        material.matcap = filler.field(block, "matcap")?;
        material.ramp = filler.field(block, "ramp")?;
        material.detail = filler.field(block, "detail")?;
        material.flow = filler.field(block, "flow")?;
        for layer in block.children_of("layer") {
            if material.layers.len() == MAX_LAYERS {
                return Err(CompileError::Bad(format!(
                    "{name} has more than {MAX_LAYERS} layers"
                )));
            }
            let mask = if layer.get("mask").and_then(Value::text) == Some("paint") {
                Some(PAINT_MASK)
            } else {
                filler.field(layer, "mask")?
            };
            let (Some(base), Some(mask)) = (filler.field(layer, "base")?, mask) else {
                return Err(CompileError::Bad(format!(
                    "{name}: a layer needs a base and a mask"
                )));
            };
            let surface = filler.layer_surface(layer, base)?;
            material.layers.push(MaterialLayer {
                base,
                mask,
                surface,
            });
        }
        material.slots = filler.slots;
        let flatten = block.get("flatten").and_then(Value::flag) == Some(true);
        let mips = if flatten {
            flatten_layers(&mut material, mips)
        } else {
            mips
        };
        let kind = match block.get("kind").and_then(Value::text) {
            None | Some("solid") => TextureKind::Solid,
            Some("cutout") => TextureKind::Cutout,
            Some("liquid") => TextureKind::Liquid,
            Some("sky") => TextureKind::Sky,
            Some("hidden") => TextureKind::Hidden,
            Some(other) => return Err(CompileError::Bad(format!("{name}: kind {other}"))),
        };
        Ok(Texture {
            name: name.to_owned(),
            kind,
            mips,
            animation: None,
            alternate: None,
            material: Some(Arc::new(material)),
        })
    }

    /// The texture of `name`, from `materials/<name>.emat`. A missing material
    /// shows a placeholder, so a map still plays and the gap is plain to see.
    ///
    /// # Errors
    ///
    /// Returns an error if a material exists but is broken.
    pub fn load(&mut self, name: &str) -> Result<Texture, CompileError> {
        self.load_as(name, self.playing)
    }

    /// The texture of `name` in plain pixels, as [`Self::load`] gives it to a
    /// bake, for whatever reads it here and not on the GPU.
    ///
    /// # Errors
    ///
    /// Returns an error if a material exists but is broken.
    pub fn load_pixels(&mut self, name: &str) -> Result<Texture, CompileError> {
        self.load_as(name, false)
    }

    /// The texture of `name`, with pictures as they were compiled if `compiled`
    /// says so.
    fn load_as(&mut self, name: &str, compiled: bool) -> Result<Texture, CompileError> {
        match self.document_named(name, 0)? {
            Some(document) => self.read_as(name, &document, compiled),
            None => Ok(placeholder_texture(name)),
        }
    }

    /// The document of the material `name`, with what it inherits filled in, or
    /// `None` if there's no such material.
    fn document_named(&self, name: &str, depth: usize) -> Result<Option<Document>, CompileError> {
        let Ok(bytes) = self.filesystem.read(&format!("materials/{name}.emat")) else {
            return Ok(None);
        };
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| CompileError::Bad(format!("{name} isn't text")))?;
        let mut document = from_text(text)?;
        let parent = document
            .blocks_of("surface")
            .next()
            .and_then(|block| block.get("parent"))
            .and_then(Value::text)
            .map(str::to_owned);
        if let Some(parent) = parent {
            if depth >= MAX_INHERITANCE {
                return Err(CompileError::Bad(format!(
                    "{name}: parents go too deep, or round in a circle"
                )));
            }
            let Some(from) = self.document_named(&parent, depth + 1)? else {
                return Err(CompileError::Bad(format!(
                    "{name}: there's no parent {parent}"
                )));
            };
            inherit(&mut document, &from);
        }
        Ok(Some(document))
    }

    /// The shader called `name`. One that's missing has no code, and shows as
    /// missing where it's drawn.
    #[must_use]
    pub fn shader(&self, name: &str) -> Arc<ShaderCode> {
        let bytes = self.filesystem.read(&format!("shaders/{name}.wgsl")).ok();
        let code = bytes.and_then(|bytes| String::from_utf8(bytes).ok());
        let code = code.or_else(|| self.graph_shader(name));
        Arc::new(ShaderCode {
            name: name.to_owned(),
            code: code.unwrap_or_default(),
        })
    }

    /// What the graph called `name` comes to, if there's one. One that's wrong is code that won't
    /// compile, and says why, so that it's shown as any broken shader is.
    fn graph_shader(&self, name: &str) -> Option<String> {
        let bytes = self
            .filesystem
            .read(&format!("shaders/{name}.egraph"))
            .ok()?;
        let text = String::from_utf8(bytes).ok()?;
        let made = read_graph(&text)
            .map_err(|problem| vec![problem])
            .and_then(|graph| graph_code(&graph));
        Some(
            made.unwrap_or_else(|problems| {
                format!("graph {name} is wrong: {}", problems.join("; "))
            }),
        )
    }
}
