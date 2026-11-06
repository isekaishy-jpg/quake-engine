//! Compiling: from a map document to what the engine runs (proposal 0067).
//!
//! May depend on `engine_core`, `engine_format`, `engine_map`. See proposal
//! 0008.

mod bake;
mod build;
mod cells;
mod derive;
mod effects;
mod error;
mod geometry;
mod light;
mod material;
mod mega;
mod mesh;
mod nav;
mod ray_grid;
mod source;
mod stamp;
mod texture;
mod tiles;
mod tree;
mod visibility;

pub use bake::{
    apply_bake, attach_side_files, bake_folder, map_hash, side_files, source_hash, write_bake,
};
pub use build::{TextureSource, compile, compile_text, read_map_text};
pub use derive::{DeriveSettings, DerivedMaps, derive_maps, encode_png};
pub use error::CompileError;
pub use geometry::{Plane3, Point};
pub use light::{BakeInput, ImageReader, bake_light, light_tracks};
pub use material::{MaterialLibrary, decode_png, document_of};
pub use mesh::{
    LoadedMesh, Mesh, MeshPart, PlacedMesh, PlacedTriangle, occluders_of, place_mesh, read_mesh,
    read_occlusion, resolve_meshes, write_occlusion,
};
pub use ray_grid::RayGrid;
pub use source::{
    Brush, BrushFace, DecalSource, EntitySource, MeshPlacement, PaintStroke, SkySource,
    StampSource, TileSource, WaterSource, WorkingMap, read_map,
};
pub use texture::placeholder_texture;
