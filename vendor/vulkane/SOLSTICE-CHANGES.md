# Local Vulkane extension

This directory vendors Vulkane 0.16.0 from its published Rust crate, including
the upstream source and license files. It is the game's Vulkan GPU library.

The local safe API extension adds `Image::new_2d_array` with explicit array
layers and mip levels, full-subresource color views and layout barriers,
per-mip/per-layer `BufferImageCopy` fields, and `Sampler::new_mipmapped` with
an explicit maximum LOD. Existing single-level image and sampler constructors
keep their former defaults. Terrain uses these APIs for six independent
original material layers and ten prepacked mip levels; no runtime mip-blit
feature or compressed format is required.

Changes are confined to `src/safe/image.rs` and `src/safe/command.rs`.
