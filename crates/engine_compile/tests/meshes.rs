//! What a mesh knows about which way it faces, and bringing models in.

use std::fs;
use std::path::PathBuf;

use engine_compile::{
    ConvexPiece, LoadedMesh, Mesh, MeshPart, MeshPlacement, TextureSource, compile, compile_known,
    document_of, fit_convex, import_gltf, place_mesh, read_geometry, read_map_text, read_mesh,
    resolve_meshes, write_geometry, write_mesh,
};
use engine_format::from_text;
use engine_map::{Map, Texture};
use std::sync::Arc;

/// A placement that leaves a mesh as it is.
fn in_place() -> MeshPlacement {
    MeshPlacement {
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
    let floats: [f32; 9 + 9 + 6] = [
        0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, // Corners.
        0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, // Normals.
        0.0, 0.0, 1.0, 0.0, 0.0, 1.0, // UVs.
    ];
    let blob: Vec<u8> = floats.iter().flat_map(|f| f.to_le_bytes()).collect();
    let json = format!(
        r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],
        "nodes":[{{"mesh":0,"name":"Thing","translation":[0,0,2]}}],
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

/// A level with `placements`, all of the mesh `test`, compiled with it placed.
fn level_of(placements: &[&str]) -> (engine_compile::WorkingMap, Map) {
    let mut text = String::from("format 1 map\nentity world { type = worldspawn }\nbrush { material = wall cuboid = -64 -64 -32 64 64 -16 }\n
");
    for placement in placements {
        text.push_str(&format!(
            "mesh {{ source = test {placement} }}
"
        ));
    }
    let mut working = read_map_text(&text).expect("reads");
    let mesh = Arc::new(folded(10.0));
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
