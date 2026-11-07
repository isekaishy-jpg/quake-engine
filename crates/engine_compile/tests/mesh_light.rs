//! Laying meshes out for light, and lighting them.

use std::sync::Arc;

use engine_compile::{
    BakeInput, Mesh, MeshPart, MeshPlacement, TextureSource, apply_bake, bake_light, compile,
    place_mesh, read_map_text, write_bake,
};
use engine_map::{AlphaMode, Face, Map, Material, MipLevel, PixelForm, Texture, TextureKind};

/// A room open to a sun.
const ROOM: &str = "format 1 map
entity world {
    type = worldspawn
    sun_intensity = 200
    ambient = 20
    light_spacing = 8
}
sky {
    zenith = 0.2 0.3 0.6
    horizon = 0.5 0.5 0.6
    ground = 0.2 0.2 0.2
    sun_direction = 0.3 0.2 1
    sun_color = 1 1 1
}
brush { material = floor cuboid = -512 -512 -32 512 512 0 }
";

/// Plain grey textures.
struct Plain;

impl TextureSource for Plain {
    fn texture(&mut self, name: &str) -> Texture {
        Texture {
            name: name.to_owned(),
            kind: TextureKind::Solid,
            mips: vec![MipLevel {
                width: 1,
                height: 1,
                rgba: vec![128, 128, 128, 255],
                form: PixelForm::Rgba,
            }],
            ..Texture::default()
        }
    }
}

/// A placement in the middle of the room.
fn placed(by_probe: bool) -> MeshPlacement {
    MeshPlacement {
        name: None,
        source: "test".to_owned(),
        origin: [0.0, 0.0, 0.0],
        angles: [0.0; 3],
        scale: [1.0; 3],
        only_seen: true,
        by_probe,
    }
}

/// A flat square of `cells` by `cells` quads, facing up, at `z` 64.
fn flat_sheet(cells: u32) -> Mesh {
    let mut points = Vec::new();
    let mut triangles = Vec::new();
    let row = cells + 1;
    for y in 0..=cells {
        for x in 0..=cells {
            points.push([f64::from(x) * 16.0, f64::from(y) * 16.0, 64.0]);
        }
    }
    for y in 0..cells {
        for x in 0..cells {
            let a = y * row + x;
            triangles.push([a, a + 1, a + row + 1]);
            triangles.push([a, a + row + 1, a + row]);
        }
    }
    Mesh {
        parts: vec![MeshPart {
            material: "stone".to_owned(),
            uvs: vec![[0.0; 2]; points.len()],
            points,
            triangles,
            ..MeshPart::default()
        }],
        ..Mesh::default()
    }
}

/// A ring wall of `sides`, standing on the floor.
fn ring(sides: u32) -> Mesh {
    let mut points = Vec::new();
    let mut triangles = Vec::new();
    for side in 0..sides {
        let angle = std::f64::consts::TAU * f64::from(side) / f64::from(sides);
        let (y, x) = angle.sin_cos();
        points.push([x * 96.0, y * 96.0, 0.0]);
        points.push([x * 96.0, y * 96.0, 80.0]);
    }
    for side in 0..sides {
        let (a, b) = (side * 2, side * 2 + 1);
        let (c, d) = (((side + 1) % sides) * 2, ((side + 1) % sides) * 2 + 1);
        triangles.push([a, c, d]);
        triangles.push([a, d, b]);
    }
    Mesh {
        parts: vec![MeshPart {
            material: "stone".to_owned(),
            uvs: vec![[0.0; 2]; points.len()],
            points,
            triangles,
            ..MeshPart::default()
        }],
        ..Mesh::default()
    }
}

/// The room with `mesh` in it, baked.
fn baked(mesh: &Mesh, by_probe: bool) -> Map {
    let mut working = read_map_text(ROOM).expect("reads");
    working.placed = vec![place_mesh(mesh, &placed(by_probe))];
    let mut map = compile(&working, &mut Plain).expect("compiles");
    let mut images = |path: &str| Err(engine_compile::CompileError::Bad(path.to_owned()));
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light(&mut map, &mut input).expect("bakes");
    map
}

/// The faces of the mesh, which come last.
fn mesh_faces(map: &Map) -> &[Face] {
    let group = map.mesh_groups[0];
    let first = group.first_face as usize;
    &map.faces[first..first + group.face_count as usize]
}

/// How many different light maps `faces` use.
fn light_maps(faces: &[Face]) -> usize {
    let mut firsts: Vec<u32> = faces
        .iter()
        .filter_map(|face| face.light.map(|light| light.first_sample))
        .collect();
    firsts.sort_unstable();
    firsts.dedup();
    firsts.len()
}

#[test]
fn a_flat_surface_of_many_triangles_is_one_light_map() {
    let map = baked(&flat_sheet(8), false);
    let faces = mesh_faces(&map);
    assert_eq!(faces.len(), 128);
    assert_eq!(light_maps(faces), 1);
    assert!(faces.iter().all(|face| face.light_axes.is_some()));
}

#[test]
fn a_ring_takes_a_few_light_maps_and_not_one_a_triangle() {
    let map = baked(&ring(48), false);
    let faces = mesh_faces(&map);
    assert_eq!(faces.len(), 96);
    let maps = light_maps(faces);
    assert!((3..=12).contains(&maps), "{maps} light maps");
}

/// The brightness of `light`'s first sample of each style, added up.
fn sample_at(map: &Map, face: &Face, across: u32, down: u32) -> u32 {
    let light = face.light.expect("lit");
    let layer = light.width * light.height;
    let at = light.first_sample + down * light.width + across;
    (0..light.style_count())
        .map(|style| {
            let sample = map.light[(at + style * layer) as usize];
            sample.iter().map(|part| u32::from(*part)).sum::<u32>()
        })
        .sum()
}

#[test]
fn a_curved_surface_has_no_step_where_triangles_meet() {
    let map = baked(&ring(48), false);
    // Along a chart's middle row, light changes a little from sample to sample.
    let face = mesh_faces(&map)
        .iter()
        .max_by_key(|face| face.light.map_or(0, |light| light.width))
        .expect("a face");
    let light = face.light.expect("lit");
    let row = light.height / 2;
    let samples: Vec<i64> = (1..light.width - 1)
        .map(|across| i64::from(sample_at(&map, face, across, row)))
        .collect();
    let biggest = samples
        .windows(2)
        .map(|pair| (pair[0] - pair[1]).abs())
        .max()
        .unwrap_or(0);
    let most = samples.iter().copied().max().unwrap_or(1).max(1);
    assert!(biggest * 4 < most, "a step of {biggest} in {most}");
}

#[test]
fn a_prop_lit_by_a_probe_has_one_sample() {
    let map = baked(&ring(48), true);
    let faces = mesh_faces(&map);
    assert_eq!(light_maps(faces), 1);
    let light = faces[0].light.expect("lit");
    assert_eq!((light.width, light.height), (1, 1));
    assert!(faces[0].lit_by_probe());
    assert!(sample_at(&map, &faces[0], 0, 0) > 0, "it isn't dark");
}

#[test]
fn a_mesh_on_its_own_light_maps_costs_less_than_one_a_triangle() {
    let map = baked(&flat_sheet(16), false);
    let faces = mesh_faces(&map);
    let light = faces[0].light.expect("lit");
    let samples = light.width * light.height;
    // Sixteen 16 unit cells at 8 units a sample, and a border.
    assert!(samples < 40 * 40, "{samples} samples");
}

#[test]
fn a_bake_read_back_keeps_a_meshs_light_map() {
    let map = baked(&flat_sheet(4), false);
    let bytes = write_bake(&map, 9);
    let mut working = read_map_text(ROOM).expect("reads");
    working.placed = vec![place_mesh(&flat_sheet(4), &placed(false))];
    let mut again = compile(&working, &mut Plain).expect("compiles");
    assert!(apply_bake(&mut again, &bytes, 9).expect("reads"));
    assert_eq!(mesh_faces(&map), mesh_faces(&again));
}

#[test]
fn a_tiled_maps_chart_keeps_its_samples_together() {
    let mut working = read_map_text(ROOM).expect("reads");
    working.tile_size = 64.0;
    working.placed = vec![place_mesh(&flat_sheet(16), &placed(false))];
    let mut map = compile(&working, &mut Plain).expect("compiles");
    let mut images = |path: &str| Err(engine_compile::CompileError::Bad(path.to_owned()));
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light(&mut map, &mut input).expect("bakes");
    let bytes = write_bake(&map, 9);
    assert!(apply_bake(&mut map, &bytes, 9).expect("reads"));
    let faces = mesh_faces(&map);
    let first = faces[0].light.expect("lit");
    assert_ne!(first.tile, engine_map::RESIDENT, "its tile holds it");
    for face in faces {
        let light = face.light.expect("lit");
        assert_eq!(
            (light.tile, light.first_sample),
            (first.tile, first.first_sample)
        );
    }
}

/// Plain textures, with one of glass.
struct WithGlass;

impl TextureSource for WithGlass {
    fn texture(&mut self, name: &str) -> Texture {
        let mut texture = Plain.texture(name);
        if name == "glass" {
            texture.material = Some(Arc::new(Material {
                alpha: AlphaMode::Blend,
                ..Material::default()
            }));
        }
        texture
    }
}

/// All the light on the room's floor, under a sheet of `material`, or none.
fn light_under(material: Option<&str>) -> u64 {
    let mut working = read_map_text(ROOM).expect("reads");
    if let Some(material) = material {
        let mut mesh = flat_sheet(4);
        mesh.parts[0].material = material.to_owned();
        let placement = MeshPlacement {
            only_seen: false,
            ..placed(false)
        };
        working.placed = vec![place_mesh(&mesh, &placement)];
    }
    let mut map = compile(&working, &mut WithGlass).expect("compiles");
    let mut images = |path: &str| Err(engine_compile::CompileError::Bad(path.to_owned()));
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light(&mut map, &mut input).expect("bakes");
    let last = map
        .mesh_groups
        .first()
        .map_or(map.faces.len(), |group| group.first_face as usize);
    map.faces[..last]
        .iter()
        .filter_map(|face| face.light)
        .map(|light| {
            let first = light.first_sample as usize;
            let count = (light.width * light.height) as usize;
            map.light[first..first + count]
                .iter()
                .flatten()
                .map(|part| u64::from(*part))
                .sum::<u64>()
        })
        .sum()
}

/// Light passes what's see-through in a mesh, as it does in a brush. A solid sheet shades the
/// floor, and glass doesn't.
#[test]
fn a_mesh_of_glass_casts_no_shadow() {
    let open = light_under(None);
    let solid = light_under(Some("solid"));
    let glass = light_under(Some("glass"));
    assert!(solid < open, "{solid} of {open}");
    assert_eq!(glass, open);
}
