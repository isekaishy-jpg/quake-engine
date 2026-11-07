//! What a mesh knows about which way it faces, and bringing models in.

use std::fs;
use std::path::PathBuf;

use engine_compile::{
    ConvexPiece, LoadedMesh, Mesh, MeshAttach, MeshPart, MeshPlacement, TextureSource, add_levels,
    compile, compile_known, document_of, fit_convex, import_gltf, place_mesh, read_geometry,
    read_map_text, read_mesh, resolve_meshes, write_geometry, write_mesh,
};
use engine_format::from_text;
use engine_map::{Map, Texture};
use std::sync::Arc;

/// A level with nothing but a floor.
const FLOOR: &str = "format 1 map\nentity world { type = worldspawn }\nbrush { material = wall cuboid = -64 -64 -32 64 64 -16 }\n\n";

/// A placement that leaves a mesh as it is.
fn in_place() -> MeshPlacement {
    MeshPlacement {
        name: None,
        source: "test".to_owned(),
        origin: [0.0; 3],
        angles: [0.0; 3],
        scale: [1.0; 3],
        only_seen: false,
        by_probe: false,
    }
}

/// A part with `points` and `triangles`.
fn part(points: Vec<[f64; 3]>, triangles: Vec<[u32; 3]>) -> MeshPart {
    MeshPart {
        uvs: vec![[0.0; 2]; points.len()],
        points,
        triangles,
        ..MeshPart::default()
    }
}

/// Two squares meeting along `x = 0`, one flat and one turned up by `degrees`.
fn folded(degrees: f64) -> Mesh {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let points = vec![
        [-1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
        [cos, 0.0, sin],
        [cos, 1.0, sin],
    ];
    let triangles = vec![[0, 1, 2], [0, 2, 3], [1, 4, 5], [1, 5, 2]];
    Mesh {
        parts: vec![part(points, triangles)],
        ..Mesh::default()
    }
}

/// How far `normal` is from straight up, as its cosine.
fn upness(normal: [f64; 3]) -> f64 {
    normal[2]
}

#[test]
fn a_gentle_fold_is_smooth_and_a_sharp_one_isnt() {
    let gentle = place_mesh(&folded(20.0), &in_place());
    let at_the_fold = gentle.triangles[0].normals[1];
    assert!(upness(at_the_fold) < 0.999, "leans: {at_the_fold:?}");
    let sharp = place_mesh(&folded(80.0), &in_place());
    let at_the_fold = sharp.triangles[0].normals[1];
    assert!(upness(at_the_fold) > 0.999, "stays flat: {at_the_fold:?}");
}

#[test]
fn normals_in_a_file_are_used() {
    let text = "format 1 mesh\npart {\n material = stone\n points = 0 0 0 1 0 0 0 1 0\n uvs = 0 0 1 0 0 1\n normals = 1 0 0 1 0 0 1 0 0\n triangles = 0 1 2\n}\n";
    let mesh = read_mesh(&from_text(text).expect("reads")).expect("a mesh");
    let placed = place_mesh(&mesh, &in_place());
    assert_eq!(placed.triangles[0].normals, [[1.0, 0.0, 0.0]; 3]);
}

#[test]
fn a_mesh_with_too_few_normals_is_refused() {
    let text = "format 1 mesh\npart {\n material = stone\n points = 0 0 0 1 0 0 0 1 0\n uvs = 0 0 1 0 0 1\n normals = 1 0 0\n triangles = 0 1 2\n}\n";
    assert!(read_mesh(&from_text(text).expect("reads")).is_err());
}

#[test]
fn stretching_leans_a_normal_the_other_way() {
    let mut mesh = folded(0.0);
    mesh.parts[0].normals = vec![[0.6, 0.0, 0.8]; 6];
    let placement = MeshPlacement {
        scale: [1.0, 1.0, 4.0],
        ..in_place()
    };
    let placed = place_mesh(&mesh, &placement);
    let [x, _, z] = placed.triangles[0].normals[0];
    assert!(x > z, "squashed against the stretch: {x} {z}");
}

/// Plain textures.
struct Plain;

impl TextureSource for Plain {
    fn texture(&mut self, name: &str) -> Texture {
        Texture {
            name: name.to_owned(),
            ..Texture::default()
        }
    }
}

/// `mesh` as the only thing in a map.
fn map_with(mesh: &Mesh) -> Map {
    let text = "format 1 map\nentity world { type = worldspawn }\nbrush { material = wall cuboid = -64 -64 -32 64 64 -16 }\n";
    let mut working = read_map_text(text).expect("reads");
    working.placed = vec![place_mesh(mesh, &in_place())];
    compile(&working, &mut Plain).expect("compiles")
}

#[test]
fn a_map_keeps_which_way_its_meshes_face() {
    let mut mesh = folded(20.0);
    mesh.parts[0].material = "stone".to_owned();
    let map = map_with(&mesh);
    assert!(map.normals.iter().any(|normal| *normal != [0; 3]));
    let bytes = write_geometry(&map, 3);
    let geometry = read_geometry(&bytes, 3).expect("reads");
    let text = "format 1 map\nentity world { type = worldspawn }\nbrush { material = wall cuboid = -64 -64 -32 64 64 -16 }\n";
    let mut working = read_map_text(text).expect("reads");
    working.placed = vec![place_mesh(&mesh, &in_place())];
    let again = compile_known(&working, &mut Plain, geometry).expect("compiles");
    assert_eq!(map.normals, again.normals);
}

/// One triangle, in glTF's axes, with a normal, as a `.glb`.
fn triangle_glb() -> Vec<u8> {
    glb_of(
        r#"[{"mesh":0,"name":"Thing","translation":[0,0,2]}]"#,
        "[0]",
    )
}

/// A `.glb` with `nodes`, each of them that triangle, and `shown` in its scene.
fn glb_of(nodes: &str, shown: &str) -> Vec<u8> {
    let floats: [f32; 9 + 9 + 6] = [
        0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, // Corners.
        0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, // Normals.
        0.0, 0.0, 1.0, 0.0, 0.0, 1.0, // UVs.
    ];
    let blob: Vec<u8> = floats.iter().flat_map(|f| f.to_le_bytes()).collect();
    let json = format!(
        r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":{shown}}}],
        "nodes":{nodes},
        "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2}},"material":0}}]}}],
        "materials":[{{"name":"Dull Gold","alphaMode":"MASK","pbrMetallicRoughness":{{"baseColorFactor":[1,0.5,0,1],"metallicFactor":1,"roughnessFactor":0.25}}}}],
        "accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}},
        {{"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"}},
        {{"bufferView":2,"componentType":5126,"count":3,"type":"VEC2"}}],
        "bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":36}},
        {{"buffer":0,"byteOffset":36,"byteLength":36}},
        {{"buffer":0,"byteOffset":72,"byteLength":24}}],
        "buffers":[{{"byteLength":{}}}]}}"#,
        blob.len()
    );
    let mut json = json.into_bytes();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let total = u32::try_from(12 + 8 + json.len() + 8 + blob.len()).expect("small");
    let mut out = Vec::new();
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2_u32.to_le_bytes());
    out.extend_from_slice(&total.to_le_bytes());
    let length = u32::try_from(json.len()).expect("small");
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    let length = u32::try_from(blob.len()).expect("small");
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&blob);
    out
}

/// A file holding `bytes`.
fn file(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("engine-mesh-{}-{name}", std::process::id()));
    fs::write(&path, bytes).expect("write");
    path
}

#[test]
fn a_gltf_model_comes_in_with_our_axes_and_scale() {
    let path = file("axes.glb", &triangle_glb());
    let model = import_gltf(&path, "thing", 10.0).expect("imports");
    let part = &model.mesh.parts[0];
    assert_eq!(part.name, "thing");
    // Y is up there and Z is here. Its node moved it two metres towards the eye.
    assert_eq!(part.points[0], [0.0, -20.0, 0.0]);
    assert_eq!(part.points[2], [0.0, -20.0, 10.0]);
    assert_eq!(part.normals[0], [0.0, -1.0, 0.0]);
    assert_eq!(part.triangles, [[0, 1, 2]]);
    fs::remove_file(&path).expect("clean up");
}

#[test]
fn a_gltf_material_says_metal_and_roughness() {
    let path = file("material.glb", &triangle_glb());
    let model = import_gltf(&path, "thing", 1.0).expect("imports");
    let (name, text) = &model.materials[0];
    assert_eq!(name, "thing_dull_gold");
    assert_eq!(model.mesh.parts[0].material, *name);
    assert!(text.contains("metallic = 1"), "{text}");
    assert!(text.contains("roughness = 0.25"), "{text}");
    assert!(text.contains("alpha = mask"), "{text}");
    let colour = text.lines().find(|line| line.contains("base = "));
    assert!(colour.expect("a base").contains("255_128_0_255"), "{text}");
    assert_eq!(model.pictures.len(), 1, "one plain picture");
    fs::remove_file(&path).expect("clean up");
}

#[test]
fn a_file_with_nothing_to_draw_is_refused() {
    let path = file(
        "empty.gltf",
        br#"{"asset":{"version":"2.0"},"scenes":[{"nodes":[]}]}"#,
    );
    let error = import_gltf(&path, "thing", 1.0).expect_err("refused");
    assert!(error.to_string().contains("nothing"), "{error}");
    fs::remove_file(&path).expect("clean up");
}

#[test]
fn a_mesh_written_reads_back() {
    let path = file("round.glb", &triangle_glb());
    let model = import_gltf(&path, "thing", 1.0).expect("imports");
    let bytes = write_mesh(&model.mesh);
    let again = read_mesh(&document_of(&bytes).expect("reads")).expect("a mesh");
    assert_eq!(again, model.mesh);
    fs::remove_file(&path).expect("clean up");
}

/// A ring wall of `sides` with a rim, open at the top and bottom, as a well is.
fn well(sides: u32) -> Mesh {
    let (outer, inner, height) = (96.0, 72.0, 80.0);
    let mut points = Vec::new();
    for side in 0..sides {
        let angle = std::f64::consts::TAU * f64::from(side) / f64::from(sides);
        let (y, x) = angle.sin_cos();
        points.push([x * outer, y * outer, 0.0]);
        points.push([x * outer, y * outer, height]);
        points.push([x * inner, y * inner, height]);
        points.push([x * inner, y * inner, 0.0]);
    }
    let mut triangles = Vec::new();
    for side in 0..sides {
        let here = side * 4;
        let there = ((side + 1) % sides) * 4;
        for ring in 0..4 {
            let next = (ring + 1) % 4;
            triangles.push([here + ring, there + ring, there + next]);
            triangles.push([here + ring, there + next, here + next]);
        }
    }
    Mesh {
        parts: vec![part(points, triangles)],
        ..Mesh::default()
    }
}

/// Whether `point` is inside any of `pieces`.
fn inside(pieces: &[ConvexPiece], point: [f64; 3]) -> bool {
    pieces.iter().any(|piece| {
        piece.iter().all(|(normal, far)| {
            normal[0] * point[0] + normal[1] * point[1] + normal[2] * point[2] <= *far
        })
    })
}

#[test]
fn a_well_can_be_stepped_into() {
    let pieces = fit_convex(&well(32));
    assert!(pieces.len() > 4, "{} pieces", pieces.len());
    assert!(pieces.len() <= 64, "{} pieces", pieces.len());
    assert!(!inside(&pieces, [0.0, 0.0, 40.0]), "its middle is open");
    assert!(!inside(&pieces, [55.0, 0.0, 40.0]), "well inside its wall");
    assert!(inside(&pieces, [84.0, 0.0, 40.0]), "its wall is solid");
    assert!(inside(&pieces, [0.0, 84.0, 40.0]), "all the way round");
    assert!(inside(&pieces, [-59.4, -59.4, 40.0]), "and at an angle");
    assert!(!inside(&pieces, [120.0, 0.0, 40.0]), "outside it");
}

#[test]
fn a_box_is_one_piece() {
    let points = vec![
        [0.0, 0.0, 0.0],
        [64.0, 0.0, 0.0],
        [64.0, 64.0, 0.0],
        [0.0, 64.0, 0.0],
        [0.0, 0.0, 64.0],
        [64.0, 0.0, 64.0],
        [64.0, 64.0, 64.0],
        [0.0, 64.0, 64.0],
    ];
    let triangles = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    let mesh = Mesh {
        parts: vec![part(points, triangles)],
        ..Mesh::default()
    };
    let pieces = fit_convex(&mesh);
    assert_eq!(pieces.len(), 1);
    assert!(inside(&pieces, [32.0, 32.0, 32.0]), "it's filled");
}

#[test]
fn a_mesh_with_no_size_has_no_pieces() {
    assert!(fit_convex(&Mesh::default()).is_empty());
}

#[test]
fn a_placed_mesh_is_collided_with_by_its_pieces() {
    let mut mesh = well(16);
    mesh.convex = fit_convex(&mesh);
    let count = mesh.convex.len();
    let placement = MeshPlacement {
        origin: [100.0, 0.0, 0.0],
        angles: [0.0, 90.0, 0.0],
        scale: [2.0, 2.0, 1.0],
        ..in_place()
    };
    let placed = place_mesh(&mesh, &placement);
    assert_eq!(placed.hulls.len(), count);
    let outward = |point: [f64; 3], brush: &engine_compile::Brush| {
        brush.faces.iter().all(|face| {
            let plane = &face.plane;
            let [x, y, z] = plane.normal;
            x * point[0] + y * point[1] + z * point[2] <= plane.distance
        })
    };
    let solid = |point: [f64; 3]| placed.hulls.iter().any(|brush| outward(point, brush));
    assert!(!solid([100.0, 0.0, 40.0]), "its middle is open");
    assert!(solid([100.0, 168.0, 40.0]), "its wall is where it was put");
}

#[test]
fn boxes_an_author_chose_win_over_pieces() {
    let mut mesh = well(16);
    mesh.convex = fit_convex(&mesh);
    mesh.hulls = vec![[0.0, 0.0, 0.0, 8.0, 8.0, 8.0]];
    assert_eq!(place_mesh(&mesh, &in_place()).hulls.len(), 1);
}

#[test]
fn pieces_survive_a_mesh_being_written() {
    let mut mesh = well(8);
    mesh.parts[0].normals = vec![[0.0, 0.0, 1.0]; mesh.parts[0].points.len()];
    mesh.convex = fit_convex(&mesh);
    let bytes = write_mesh(&mesh);
    let again = read_mesh(&document_of(&bytes).expect("reads")).expect("a mesh");
    assert_eq!(again.convex, mesh.convex);
}

/// A level with `placements`, all of `mesh`, compiled with it placed.
fn level_with(mesh: Mesh, placements: &[&str]) -> (engine_compile::WorkingMap, Map) {
    let mut text = String::from(FLOOR);
    for placement in placements {
        text.push_str(&format!("mesh {{ source = test {placement} }}\n"));
    }
    let mut working = read_map_text(&text).expect("reads");
    let mesh = Arc::new(mesh);
    resolve_meshes(&mut working, &mut |_| {
        Ok(LoadedMesh {
            mesh: Arc::clone(&mesh),
            occluders: Vec::new(),
        })
    })
    .expect("places");
    let map = compile(&working, &mut Plain).expect("compiles");
    (working, map)
}

/// A level with `placements`, all of a folded square.
fn level_of(placements: &[&str]) -> (engine_compile::WorkingMap, Map) {
    level_with(folded(10.0), placements)
}

/// Copies lit from one place share one mesh, however many there are, and each keeps its own
/// place and faces.
#[test]
fn copies_of_a_mesh_share_its_triangles() {
    let (_, map) = level_of(&[
        "origin = [0 0 0] light = probe",
        "origin = [100 0 0] light = probe",
        "origin = [200 0 0] light = probe",
    ]);
    assert_eq!(map.shared_meshes.len(), 1);
    assert_eq!(map.mesh_instances.len(), 3);
    assert_eq!(map.shared_meshes[0].indices.len(), 12);
    assert!((map.mesh_instances[2].rows[0][3] - 200.0).abs() < 0.001);
    let faces: Vec<u32> = map
        .mesh_instances
        .iter()
        .map(|copy| copy.first_face)
        .collect();
    assert!(faces.windows(2).all(|pair| pair[0] < pair[1]), "{faces:?}");
}

/// What has a light map of its own stays in the world, so levels look as they did.
#[test]
fn a_mesh_with_its_own_light_map_stays_baked() {
    let (_, map) = level_of(&["origin = [0 0 0]", "origin = [100 0 0] light = probe"]);
    assert_eq!(map.mesh_instances.len(), 1);
    assert!((map.mesh_instances[0].rows[0][3] - 100.0).abs() < 0.001);
}

/// An instance's matrix does what placing its triangles did, turned and stretched.
#[test]
fn an_instance_lands_where_its_faces_are() {
    let (_, map) = level_of(&["origin = [10 20 30] angles = [0 90 0] scale = 2 light = probe"]);
    let instance = map.mesh_instances[0];
    let shared = &map.shared_meshes[0];
    let face = &map.faces[instance.first_face as usize];
    let baked = map.face_points(face);
    let first = shared.indices[..3].iter().map(|&index| {
        let [x, y, z] = shared.corners[index as usize].position;
        instance.place(engine_core::math::Vec3::new(x, y, z))
    });
    for (made, kept) in first.zip(baked) {
        assert!((made - *kept).length() < 0.01, "{made:?} against {kept:?}");
    }
}

/// A level read from its kept geometry has the same instances as a whole compile.
#[test]
fn kept_geometry_keeps_its_instances() {
    let (working, map) = level_of(&["origin = [0 0 0] light = probe", "origin = [50 0 0]"]);
    let geometry = read_geometry(&write_geometry(&map, 7), 7).expect("reads");
    let again = compile_known(&working, &mut Plain, geometry).expect("compiles");
    assert_eq!(again.mesh_instances, map.mesh_instances);
    assert_eq!(again.shared_meshes, map.shared_meshes);
}

/// An author's own levels, by name, are used, and are kept apart from the finest. A level that
/// was skipped is the one before it, and one with no finest part to go to is left out.
#[test]
fn an_authors_levels_join_the_part_they_are_levels_of() {
    let nodes = r#"[{"mesh":0,"name":"Thing"},
        {"mesh":0,"name":"Thing_lod2","translation":[5,0,0]},
        {"mesh":0,"name":"Lost_lod1","translation":[9,0,0]}]"#;
    let path = file("levels.glb", &glb_of(nodes, "[0,1,2]"));
    let model = import_gltf(&path, "thing", 1.0).expect("imports");
    fs::remove_file(&path).expect("clean up");
    let parts = &model.mesh.parts;
    assert_eq!(parts.len(), 1, "{parts:?}");
    let part = &parts[0];
    assert_eq!(part.triangles, [[0, 1, 2]], "the finest is as it was");
    assert_eq!(part.levels.len(), 2);
    assert_eq!(part.levels[0], part.triangles, "level 1 was skipped");
    assert_eq!(part.levels[1], [[3, 4, 5]], "level 2 has its own corners");
    assert_eq!(part.points.len(), 6);
    assert_eq!(part.normals.len(), 6);
    assert_eq!(part.points[3][0], 5.0, "and they're where it put them");
}

/// Coarser levels are fewer triangles on the same corners, and survive being written.
#[test]
fn coarser_levels_share_corners_and_survive_a_file() {
    let mut mesh = well(128);
    let levels = add_levels(&mut mesh);
    assert!(levels >= 1, "made none");
    let part = &mesh.parts[0];
    let mut last = part.triangles.len();
    for level in &part.levels {
        assert!(level.len() < last, "{} then {last}", level.len());
        last = level.len();
        assert!(
            level
                .iter()
                .flatten()
                .all(|c| (*c as usize) < part.points.len())
        );
    }
    let back = read_mesh(&document_of(&write_mesh(&mesh)).expect("reads")).expect("is a mesh");
    assert_eq!(back.parts[0].levels, part.levels);
}

/// What's already small isn't made smaller.
#[test]
fn a_small_mesh_gets_no_levels() {
    let mut mesh = folded(20.0);
    assert_eq!(add_levels(&mut mesh), 0);
    assert!(mesh.parts[0].normals.is_empty());
}

/// Simplifying goes by the mesh as it's joined. Going by normals first splits corners wherever
/// smoothing differs and leaves nothing to simplify. A real model shows that, where a clean
/// generated one doesn't, so this is a real one.
#[test]
fn a_real_model_with_no_normals_simplifies() {
    let bytes = include_bytes!("../../../content/meshes/vesper_fall.emesh");
    let mut mesh = read_mesh(&document_of(bytes).expect("reads")).expect("is a mesh");
    let count = |mesh: &Mesh, level: usize| -> usize {
        let at = |part: &MeshPart| match level {
            0 => part.triangles.len(),
            _ => part.levels.get(level - 1).map_or(0, Vec::len),
        };
        mesh.parts.iter().map(at).sum()
    };
    assert_eq!(add_levels(&mut mesh), 3);
    let finest = count(&mesh, 0);
    assert!(
        count(&mesh, 1) * 10 < finest * 6,
        "{finest} to {}",
        count(&mesh, 1)
    );
    assert!(
        count(&mesh, 3) * 10 < finest * 2,
        "{finest} to {}",
        count(&mesh, 3)
    );
}

/// With no normals in the file, a level's are worked out from its own triangles, so a coarser
/// level doesn't take the shape of a finer one's.
#[test]
fn a_level_works_out_normals_from_its_own_shape() {
    let text = "format 1 mesh\npart { material = a points = [0 0 0 1 0 0 0 1 0 1 1 1] uvs = [0 0 1 0 0 1 1 1] triangles = [0 1 2 1 3 2] level_1 = [0 1 2] }\n";
    let mesh = read_mesh(&from_text(text).expect("reads")).expect("is a mesh");
    assert_eq!(mesh.parts[0].levels.len(), 1);
    let (_, map) = level_with(mesh, &["origin = [0 0 0] light = probe"]);
    let shared = &map.shared_meshes[0];
    let flat = shared.levels[1][0];
    let corner = shared.indices[flat.first as usize] as usize;
    assert_eq!(
        shared.corners[corner].normal,
        [0, 0, 127],
        "flat on its own"
    );
}

/// A shared mesh's levels share one set of corners, each with its own parts.
#[test]
fn a_shared_mesh_keeps_its_levels() {
    let mut mesh = well(128);
    let levels = add_levels(&mut mesh);
    let (_, map) = level_with(mesh, &["origin = [0 0 0] light = probe"]);
    let shared = &map.shared_meshes[0];
    assert_eq!(shared.levels.len(), levels + 1);
    assert_eq!(shared.level_sizes, [0.25, 0.1, 0.03]);
    let triangles =
        |parts: &Vec<engine_map::SharedPart>| -> u32 { parts.iter().map(|part| part.count).sum() };
    let counts: Vec<u32> = shared.levels.iter().map(triangles).collect();
    assert!(
        counts.windows(2).all(|pair| pair[1] < pair[0]),
        "{counts:?}"
    );
}

/// A mesh with a place on it, a unit up and two along, facing along its own +X.
fn with_a_place() -> Mesh {
    let mut mesh = folded(10.0);
    mesh.attach.push(MeshAttach {
        name: "spout".to_owned(),
        origin: [2.0, 0.0, 1.0],
        forward: [1.0, 0.0, 0.0],
        left: [0.0, 1.0, 0.0],
    });
    mesh
}

/// Turning and undoing it comes back to the same angles, whichever way it's turned.
#[test]
fn a_place_turns_with_its_mesh() {
    for angles in [
        [0.0, 0.0, 0.0],
        [0.0, 90.0, 0.0],
        [30.0, 45.0, 0.0],
        [0.0, 0.0, 40.0],
        [-20.0, 130.0, 25.0],
    ] {
        let placement = MeshPlacement {
            angles,
            ..in_place()
        };
        let point = &place_mesh(&with_a_place(), &placement).attach[0];
        let turned = [point.angles.x, point.angles.y, point.angles.z].map(f64::from);
        for (got, wanted) in turned.iter().zip(angles) {
            assert!((got - wanted).abs() < 0.01, "{turned:?} against {angles:?}");
        }
    }
}

/// A place goes where its mesh goes, is stretched with it, and isn't turned by the stretch.
#[test]
fn a_place_is_where_its_mesh_puts_it() {
    let placement = MeshPlacement {
        origin: [100.0, 0.0, 0.0],
        angles: [0.0, 90.0, 0.0],
        scale: [3.0; 3],
        ..in_place()
    };
    let point = &place_mesh(&with_a_place(), &placement).attach[0];
    // Two along, stretched to six, and turned to point along +Y.
    assert!((point.origin.x - 100.0).abs() < 0.001, "{point:?}");
    assert!((point.origin.y - 6.0).abs() < 0.001, "{point:?}");
    assert!((point.origin.z - 3.0).abs() < 0.001, "{point:?}");
    assert!((point.angles.y - 90.0).abs() < 0.01, "{point:?}");
}

#[test]
fn places_survive_a_file() {
    let mesh = with_a_place();
    let back = read_mesh(&document_of(&write_mesh(&mesh)).expect("reads")).expect("is a mesh");
    assert_eq!(back.attach, mesh.attach);
}

/// An empty named for it is a place, with our axes, and draws nothing.
#[test]
fn an_empty_in_a_model_is_a_place() {
    let nodes = r#"[{"mesh":0,"name":"Thing"},
        {"name":"attach_Muzzle","translation":[0,1,2]}]"#;
    let path = file("attach.glb", &glb_of(nodes, "[0,1]"));
    let model = import_gltf(&path, "thing", 10.0).expect("imports");
    fs::remove_file(&path).expect("clean up");
    assert_eq!(model.mesh.parts.len(), 1);
    let point = &model.mesh.attach[0];
    assert_eq!(point.name, "muzzle");
    // Y up becomes Z up, and two towards the eye goes to -Y, at ten units a metre.
    assert_eq!(point.origin, [0.0, -20.0, 10.0]);
    assert_eq!(point.forward, [1.0, 0.0, 0.0]);
    assert_eq!(point.left, [0.0, 1.0, 0.0]);
}

/// A level with `blocks`, its meshes all `with_a_place`.
fn level_of_blocks(blocks: &str) -> Result<Map, engine_compile::CompileError> {
    let text = format!("{FLOOR}{blocks}\n");
    let mut working = read_map_text(&text).expect("reads");
    let mesh = Arc::new(with_a_place());
    resolve_meshes(&mut working, &mut |_| {
        Ok(LoadedMesh {
            mesh: Arc::clone(&mesh),
            occluders: Vec::new(),
        })
    })?;
    compile(&working, &mut Plain)
}

/// The value of `key` on the first entity of `kind` in `map`.
fn field_of(map: &Map, kind: &str, key: &str) -> Option<String> {
    let found = map
        .entities
        .iter()
        .find(|entity| entity.get("classname") == Some(kind))?;
    found.get(key).map(str::to_owned)
}

#[test]
fn a_named_place_is_kept_and_found() {
    let map = level_of_blocks("point { name = door origin = [1 2 3] angles = [0 90 0] }")
        .expect("compiles");
    let place = map.place("door").expect("there");
    assert_eq!(place.origin, engine_core::math::Vec3::new(1.0, 2.0, 3.0));
    assert!((place.angles.y - 90.0).abs() < 0.001);
    assert!(map.place("window").is_none());
}

/// A named placement's places are named after it, so two of the same mesh can be told apart.
#[test]
fn a_named_placement_names_its_places() {
    let map = level_of_blocks(
        "mesh { source = test name = left origin = [0 0 0] }
         mesh { source = test name = right origin = [100 0 0] }",
    )
    .expect("compiles");
    let left = map.place("left.spout").expect("there");
    let right = map.place("right.spout").expect("there");
    assert!((right.origin.x - left.origin.x - 100.0).abs() < 0.001);
}

/// A mesh entity's places go with its model, for what follows it.
#[test]
fn a_mesh_entity_has_places_on_its_model() {
    let map = level_of_blocks("entity lift { type = func_door name = lift mesh = test }")
        .expect("compiles");
    assert_eq!(map.model_points.len(), 1);
    let model = field_of(&map, "func_door", "model").expect("has one");
    let number: u32 = model.trim_start_matches('*').parse().expect("a number");
    assert!(map.model_point(number, "spout").is_some());
    assert!(map.model_point(number, "nothing").is_none());
    assert!(map.model_point(number + 1, "spout").is_none());
}

/// An entity with `attach` starts there.
#[test]
fn an_attached_entity_starts_at_its_place() {
    let map = level_of_blocks(
        "point { name = spot origin = [7 8 9] }
         entity thing { type = light attach = spot }",
    )
    .expect("compiles");
    let origin = field_of(&map, "light", "origin").expect("has one");
    assert_eq!(origin, "7 8 9");
}

/// Two copies of a prefab each find their own place, with the same name in both.
#[test]
fn copies_of_a_prefab_find_their_own_places() {
    let map = level_of_blocks(
        "point { name = spout origin = [1 0 0] copy = 10 }
         point { name = spout origin = [2 0 0] copy = 20 }
         entity thing { type = light attach = spout copy = 20 }",
    )
    .expect("compiles");
    assert_eq!(field_of(&map, "light", "origin").expect("has one"), "2 0 0");
}

/// A place that isn't there is a mistake, and it says so, or a thing would quietly sit at zero.
#[test]
fn attaching_to_nothing_is_refused() {
    let error = level_of_blocks("entity thing { type = light attach = nowhere }");
    let error = error.expect_err("refused").to_string();
    assert!(error.contains("nowhere"), "{error}");
}

/// Levels share what's identical in their trees, which makes them graphs. Something turned
/// crosses the cells that visibility cuts space into, and if those are copied as trees, each
/// shared part is copied once for every way to reach it. One barrel at 45 degrees took 560 MB
/// of this very level. A level made up for a test has too little in it to show that.
#[test]
fn something_turned_does_not_blow_up_a_real_level() {
    let text = include_str!("../../../content/maps/proving_ground.emap");
    let barrel = include_bytes!("../../../content/meshes/barrel.emesh");
    let mut barrel = read_mesh(&document_of(barrel).expect("reads")).expect("is a mesh");
    barrel.convex = fit_convex(&barrel);
    let barrel = Arc::new(barrel);
    let mut working = read_map_text(text).expect("reads");
    resolve_meshes(&mut working, &mut |_| {
        Ok(LoadedMesh {
            mesh: Arc::clone(&barrel),
            occluders: Vec::new(),
        })
    })
    .expect("places");
    let map = compile(&working, &mut Plain).expect("compiles");
    let (nodes, clip) = (map.tree.nodes.len(), map.tree.clip_nodes.len());
    // About 4,000 and 11,000. Copying cells as trees makes the first 30,000, and fitting pieces
    // finer than can be felt makes the second 2,600,000.
    assert!(nodes < 10_000, "{nodes} nodes");
    assert!(clip < 50_000, "{clip} clip nodes");
}

/// An arm of three bones from Blender, that waves, with a place on its tip and a shape key.
fn rigged() -> Mesh {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/rig_test.glb");
    import_gltf(&path, "arm", 32.0).expect("imports").mesh
}

/// The tallest corner of `mesh` after `palette`, skinned as the engine does it.
fn tallest_skinned(mesh: &Mesh, palette: &[engine_map::Rows]) -> engine_core::math::Vec3 {
    let mut tallest = engine_core::math::Vec3::new(0.0, 0.0, f32::MIN);
    let part = &mesh.parts[0];
    for ((point, joints), weights) in part.points.iter().zip(&part.joints).zip(&part.weights) {
        #[expect(clippy::cast_possible_truncation, reason = "meshes draw in f32")]
        let corner =
            engine_core::math::Vec3::new(point[0] as f32, point[1] as f32, point[2] as f32);
        let mut skinned = engine_core::math::Vec3::ZERO;
        for (joint, weight) in joints.iter().zip(weights) {
            let moved = engine_map::moved(&palette[usize::from(*joint)], corner);
            skinned = skinned + moved * (f32::from(*weight) / 255.0);
        }
        if skinned.z > tallest.z {
            tallest = skinned;
        }
    }
    tallest
}

#[test]
fn a_skeleton_comes_in_from_blender() {
    let mesh = rigged();
    let skeleton = mesh.skeleton.as_ref().expect("has one");
    // The three bones, and the rig above them, which moves them as a joint does.
    let names: Vec<&str> = skeleton
        .joints
        .iter()
        .map(|joint| joint.name.as_str())
        .collect();
    assert_eq!(names, ["rig", "root", "middle", "end"]);
    for (at, joint) in skeleton.joints.iter().enumerate() {
        assert!(
            joint.parent.is_none_or(|parent| usize::from(parent) < at),
            "parents first"
        );
    }
    assert_eq!(skeleton.clips.len(), 1);
    assert!(
        (skeleton.clips[0].length - 2.0).abs() < 0.02,
        "{}",
        skeleton.clips[0].length
    );
    assert!(skeleton.reach > 10.0, "{}", skeleton.reach);
}

#[test]
fn each_corner_of_a_skin_comes_to_a_whole() {
    let mesh = rigged();
    let part = &mesh.parts[0];
    assert_eq!(part.joints.len(), part.points.len());
    for weights in &part.weights {
        let total: u32 = weights.iter().copied().map(u32::from).sum();
        assert_eq!(total, 255, "{weights:?}");
    }
}

/// A skinned mesh at rest is where it was made, with no joint moving it, in our axes.
#[test]
fn at_rest_a_skinned_mesh_stands_as_it_was_made() {
    let mesh = rigged();
    let skeleton = mesh.skeleton.as_ref().expect("has one");
    let palette = skeleton.palette(&skeleton.placed(&skeleton.pose(None, 0.0)));
    let tallest = tallest_skinned(&mesh, &palette);
    // Three metres tall, at 32 units to the metre, and up is our Z.
    assert!((tallest.z - 96.0).abs() < 0.5, "{tallest:?}");
    // On its top ring, a third of a metre out.
    assert!(
        (tallest.x.hypot(tallest.y) - 11.2).abs() < 0.5,
        "{tallest:?}"
    );
}

/// It bends the way Blender does: about the bones' own X, in the clip's first third.
#[test]
fn a_clip_bends_a_skinned_mesh() {
    let mesh = rigged();
    let skeleton = mesh.skeleton.as_ref().expect("has one");
    let at = |seconds: f32| {
        let placed = skeleton.placed(&skeleton.pose(Some(0), seconds));
        tallest_skinned(&mesh, &skeleton.palette(&placed))
    };
    let still = at(0.0);
    let bent = at(0.5);
    assert!((still.z - 96.0).abs() < 0.5, "it starts upright: {still:?}");
    assert!(bent.z < still.z - 5.0, "it's come down: {bent:?}");
    assert!(bent.y.abs() > 5.0, "and over to one side: {bent:?}");
    assert!(bent.x.abs() < 1.0, "and only to that side: {bent:?}");
    let other = at(1.5);
    assert!(
        other.y * bent.y < 0.0,
        "and later to the other: {other:?} after {bent:?}"
    );
}

/// A place on a bone goes with it, and it comes in on that bone.
#[test]
fn a_place_on_a_bone_goes_with_it() {
    let mesh = rigged();
    let skeleton = mesh.skeleton.as_ref().expect("has one");
    let place = skeleton
        .places
        .iter()
        .find(|place| place.name == "tip")
        .expect("there");
    let end = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == "end")
        .expect("there");
    assert_eq!(place.joint.map(usize::from), Some(end));
    let tip = |seconds: f32| {
        let placed = skeleton.placed(&skeleton.pose(Some(0), seconds));
        let rows = skeleton.place("tip", &placed).expect("there");
        engine_map::moved(&rows, engine_core::math::Vec3::ZERO)
    };
    assert!(
        (tip(0.5) - tip(0.0)).length() > 10.0,
        "{:?} from {:?}",
        tip(0.5),
        tip(0.0)
    );
}

#[test]
fn a_skeleton_survives_a_file() {
    let mesh = rigged();
    let back = read_mesh(&document_of(&write_mesh(&mesh)).expect("reads")).expect("is a mesh");
    assert_eq!(back.parts[0].joints, mesh.parts[0].joints);
    assert_eq!(back.parts[0].weights, mesh.parts[0].weights);
    let (kept, was) = (back.skeleton.expect("kept"), mesh.skeleton.expect("was"));
    assert_eq!(kept.joints.len(), was.joints.len());
    assert_eq!(kept.joints[2].parent, was.joints[2].parent);
    assert_eq!(kept.clips[0].channels.len(), was.clips[0].channels.len());
    assert_eq!(
        kept.clips[0].channels[1].path,
        was.clips[0].channels[1].path
    );
    assert_eq!(kept.places.len(), was.places.len());
    let (pose, rest) = (kept.pose(Some(0), 0.5), was.pose(Some(0), 0.5));
    for (a, b) in pose.iter().zip(&rest) {
        for (a, b) in a.rotation.iter().zip(&b.rotation) {
            assert!((a - b).abs() < 0.0001);
        }
    }
}

/// A skin that names a joint the file doesn't have is refused, not drawn with garbage.
#[test]
fn a_skin_with_no_such_joint_is_refused() {
    let text = "format 1 mesh\npart { material = a points = [0 0 0] uvs = [0 0] triangles = [] joints = [3 0 0 0] weights = [255 0 0 0] }\n";
    assert!(read_mesh(&from_text(text).expect("reads")).is_err());
}

/// The shape key comes in by name, and only what it moves is kept.
#[test]
fn a_shape_key_comes_in_from_blender() {
    let mesh = rigged();
    let skeleton = mesh.skeleton.as_ref().expect("has one");
    assert_eq!(skeleton.morph_names, ["swell"]);
    let part = &mesh.parts[0];
    assert_eq!(part.morphs.len(), 1);
    let moves = &part.morphs[0];
    assert!(!moves.is_empty());
    assert!(
        moves.len() < part.points.len(),
        "{} of {}",
        moves.len(),
        part.points.len()
    );
}

/// It swells round the middle, outwards, at the scale of the rest, and not up the arm.
#[test]
fn a_shape_key_is_in_our_axes_and_scale() {
    let mesh = rigged();
    let part = &mesh.parts[0];
    let widest = part.morphs[0]
        .iter()
        .max_by(|a, b| {
            let out = |one: &&engine_compile::MorphMove| one.position[0].hypot(one.position[1]);
            out(a).total_cmp(&out(b))
        })
        .expect("some");
    let place = part.points[widest.corner as usize];
    assert!(
        (place[2] - 48.0).abs() < 8.0,
        "widest near the middle: {place:?}"
    );
    // At the middle it doubles a radius of 11.2 units.
    let out = widest.position[0].hypot(widest.position[1]);
    assert!((out - 11.2).abs() < 1.0, "{out}");
    assert!(widest.position[2].abs() < 0.01, "{:?}", widest.position);
}

#[test]
fn morphs_survive_a_file() {
    let mesh = rigged();
    let back = read_mesh(&document_of(&write_mesh(&mesh)).expect("reads")).expect("is a mesh");
    assert_eq!(back.parts[0].morphs.len(), 1);
    assert_eq!(back.parts[0].morphs[0].len(), mesh.parts[0].morphs[0].len());
    assert_eq!(
        back.parts[0].morphs[0][0].corner,
        mesh.parts[0].morphs[0][0].corner
    );
    let (kept, was) = (back.skeleton.expect("kept"), mesh.skeleton.expect("was"));
    assert_eq!(kept.morph_names, was.morph_names);
    assert_eq!(kept.rest_weights, was.rest_weights);
}

/// A morph that names no corner is refused, not read past the end of a list.
#[test]
fn a_morph_with_no_such_corner_is_refused() {
    let text = "format 1 mesh\npart { material = a points = [0 0 0] uvs = [0 0] triangles = [] morph_0 = [5 1 0 0 0 0 0] }\n";
    assert!(read_mesh(&from_text(text).expect("reads")).is_err());
}

/// A file's morph weights are part of how it looks, so a copy that doesn't move shows them too.
#[test]
fn what_is_placed_shows_its_rest_weights() {
    let mesh = rigged();
    let skeleton = mesh.skeleton.as_ref().expect("it has one");
    assert_eq!(
        skeleton.rest_weights,
        [1.0],
        "the test file's shape key is on"
    );
    let part = &mesh.parts[0];
    let change = part.morphs[0]
        .iter()
        .max_by(|a, b| a.position[0].abs().total_cmp(&b.position[0].abs()))
        .expect("it moves something");
    assert!(change.position[0].abs() > 1.0, "{change:?}");
    let was = part.points[change.corner as usize];
    let shown = [0, 1, 2].map(|axis| was[axis] + change.position[axis]);
    let (_, map) = level_with(mesh.clone(), &["origin = [0 0 0] light = probe"]);
    let shared = &map.shared_meshes[0];
    assert!(shared.morphs.is_empty(), "there's nothing to morph it");
    let near = |place: [f64; 3]| {
        shared.corners.iter().any(|corner| {
            (0..3).all(|axis| (f64::from(corner.position[axis]) - place[axis]).abs() < 0.01)
        })
    };
    assert!(near(shown), "it's where the shape key puts it");
    assert!(!near(was), "and not where it was");
}
