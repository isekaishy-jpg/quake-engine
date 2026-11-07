//! Compiling: from a map document to what the engine runs (proposal 0067).
//!
//! May depend on `engine_core`, `engine_format`, `engine_map`. See proposal
//! 0008.

mod bake;
mod build;
mod cells;
mod charts;
mod compiled;
mod content_pack;
mod convex;
mod derive;
mod effect_read;
mod effects;
mod error;
mod geometry;
mod gltf_import;
mod gltf_skin;
mod land_pictures;
mod light;
mod material;
mod mega;
mod mesh;
mod nav;
#[cfg(feature = "packing")]
mod packing;
mod ray_grid;
#[cfg(feature = "packing")]
mod simplify;
mod source;
mod stamp;
mod terrain;
mod texture;
mod texture_check;
mod texture_pack;
mod tiles;
mod tree;
mod visibility;

pub use bake::{
    apply_bake, attach_side_files, bake_folder, map_hash, side_files, source_hash, write_bake,
};
pub use build::{TextureSource, compile, compile_known, compile_text, read_map_text};
pub use compiled::{Geometry, read_geometry, write_geometry};
pub use content_pack::{PlayMap, play_map};
pub use convex::{ConvexPiece, fit_convex};
pub use derive::{DeriveSettings, DerivedMaps, derive_maps, encode_png};
pub use effect_read::read_effect;
pub use error::CompileError;
pub use geometry::{Plane3, Point};
pub use gltf_import::{ImportedModel, import_gltf, write_mesh};
pub use land_pictures::pack_land;
pub use light::{BakeInput, ImageReader, bake_light, light_tracks};
pub use material::{MaterialLibrary, decode_png, document_of};
pub use mesh::{
    LoadedMesh, Mesh, MeshAttach, MeshPart, MorphMove, PlacedMesh, PlacedTriangle, angles_of,
    occluders_of, place_mesh, read_mesh, read_occlusion, resolve_meshes, write_occlusion,
};
#[cfg(feature = "packing")]
pub use packing::{compile_shaders, pack_content};
pub use ray_grid::RayGrid;
#[cfg(feature = "packing")]
pub use simplify::add_levels;
pub use source::{
    Brush, BrushFace, DecalSource, EntitySource, MeshPlacement, PaintStroke, SkySource,
    StampSource, TileSource, WaterSource, WorkingMap, read_map,
};
pub use terrain::{TerrainSource, read_terrain_data, write_terrain};
pub use texture::placeholder_texture;
pub use texture_check::{Repeat, check_picture};
pub use texture_pack::{
    Role, bytes_hash, pack_picture, pack_textures, packed_path, picture_roles, read_packed,
};
