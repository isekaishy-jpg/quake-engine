//! Shader graphs, as files and as the code they come to (proposal 0145).

use std::collections::BTreeMap;

use engine_assets::{DirectoryArchive, FileSystem};
use engine_compile::{
    GRAPH_KINDS, GraphInput, GraphNode, MaterialLibrary, ShaderGraph, graph_code, read_graph,
    write_graph,
};
use engine_shaders::{check_field_code, check_particle_code, check_post_code, check_surface_code};

/// A node of `kind`, with `inputs` that name nodes.
fn node(id: &str, kind: &str, inputs: &[(&str, &str)]) -> GraphNode {
    GraphNode {
        id: id.to_owned(),
        kind: kind.to_owned(),
        inputs: inputs
            .iter()
            .map(|(name, other)| ((*name).to_owned(), GraphInput::Node((*other).to_owned())))
            .collect(),
        ..GraphNode::default()
    }
}

/// `nodes`, with `outputs`.
fn graph(nodes: Vec<GraphNode>, outputs: &[(&str, &str)]) -> ShaderGraph {
    ShaderGraph {
        nodes,
        outputs: outputs
            .iter()
            .map(|(name, id)| ((*name).to_owned(), (*id).to_owned()))
            .collect(),
    }
}

/// Everything that's wrong with `graph`, as one line.
fn problems(graph: &ShaderGraph) -> String {
    graph_code(graph).expect_err("it's wrong").join("; ")
}

/// A pulse on a surface: its base, brighter with time, with a noisy glow.
fn pulse() -> ShaderGraph {
    let mut brightness = node("bright", "number", &[]);
    brightness
        .settings
        .insert("value".to_owned(), "0.5".to_owned());
    graph(
        vec![
            node("time", "time", &[]),
            node("wave", "sin", &[("a", "time")]),
            node("base", "base", &[]),
            node("lit", "multiply", &[("a", "base"), ("b", "wave")]),
            node("noise", "noise", &[]),
            brightness,
            node("glow", "multiply", &[("a", "noise"), ("b", "bright")]),
        ],
        &[("base", "lit"), ("emissive", "glow")],
    )
}

#[test]
fn a_graph_is_kept_in_a_file() {
    let mut made = pulse();
    made.nodes[0].at = [120.0, -40.0];
    made.nodes[3]
        .inputs
        .insert("b".to_owned(), GraphInput::Number(0.25));
    let mut code = node("own", "code", &[("a", "time")]);
    code.settings
        .insert("code".to_owned(), "a * vec3<f32>(1.0, 0.5, 0.0)".to_owned());
    code.settings.insert("gives".to_owned(), "vec3".to_owned());
    made.nodes.push(code);
    let again = read_graph(&write_graph(&made)).expect("reads");
    assert_eq!(again, made);
}

#[test]
fn a_graph_comes_to_code_a_surface_can_use() {
    let code = graph_code(&pulse()).expect("it's right");
    assert!(code.contains("fn shader_surface"), "{code}");
    assert!(!code.contains("fn shader_uv"), "it leaves that alone");
    check_surface_code(&code).unwrap_or_else(|problem| panic!("{problem}\n{code}"));
}

#[test]
fn each_output_lands_in_its_own_hook() {
    let made = graph(
        vec![
            node("world", "world", &[]),
            node("time", "time", &[]),
            node("sway", "add", &[("a", "world"), ("b", "time")]),
            node("uv", "uv", &[]),
            node("moved", "add", &[("a", "uv"), ("b", "time")]),
            node("tint", "image", &[("at", "moved")]),
        ],
        &[("vertex", "sway"), ("uv", "moved"), ("base", "tint")],
    );
    let code = graph_code(&made).expect("it's right");
    for hook in ["fn shader_vertex", "fn shader_uv", "fn shader_surface"] {
        assert!(code.contains(hook), "{hook}: {code}");
    }
    check_surface_code(&code).unwrap_or_else(|problem| panic!("{problem}\n{code}"));
}

#[test]
fn a_graph_can_be_a_final_pass() {
    let made = graph(
        vec![
            node("color", "color", &[]),
            node("flipped", "one_minus", &[("a", "color")]),
        ],
        &[("color", "flipped")],
    );
    let code = graph_code(&made).expect("it's right");
    check_post_code(&code).unwrap_or_else(|problem| panic!("{problem}\n{code}"));
}

#[test]
fn every_kind_compiles_where_it_can_be_read() {
    for kind in GRAPH_KINDS {
        if matches!(kind.name, "color" | "cell" | "last" | "tinted" | "life") {
            continue;
        }
        let mut made = node("it", kind.name, &[]);
        let settings = [
            ("code", "a + b"),
            ("gives", "number"),
            ("part", "x"),
            ("value", "2"),
            ("index", "1"),
        ];
        made.settings = settings
            .iter()
            .filter(|(name, _)| kind.settings.contains(name))
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        // What takes more than a number is given more.
        let wide = node("wide", "world", &[]);
        for (input, _) in kind
            .inputs
            .iter()
            .filter(|_| matches!(kind.name, "normalize" | "dot" | "length" | "split"))
        {
            made.inputs
                .insert((*input).to_owned(), GraphInput::Node("wide".to_owned()));
        }
        let made = graph(vec![wide, made], &[("emissive", "it")]);
        let code = graph_code(&made).unwrap_or_else(|why| panic!("{}: {why:?}", kind.name));
        check_surface_code(&code).unwrap_or_else(|problem| {
            panic!("{}: {problem}\n{code}", kind.name);
        });
    }
}

#[test]
fn types_widen_and_narrow_to_fit() {
    let made = graph(
        vec![
            node("uv", "uv", &[]),
            node("time", "time", &[]),
            node("base", "base", &[]),
            // A number, two and four together come to four.
            node("sum", "add", &[("a", "uv"), ("b", "base")]),
            node("more", "multiply", &[("a", "sum"), ("b", "time")]),
        ],
        // Four goes into three, and into two.
        &[("emissive", "more"), ("uv", "uv")],
    );
    let code = graph_code(&made).expect("it's right");
    check_surface_code(&code).unwrap_or_else(|problem| panic!("{problem}\n{code}"));
}

#[test]
fn a_node_can_be_typed_in_code() {
    let mut own = node("own", "code", &[("a", "time")]);
    own.settings.insert(
        "code".to_owned(),
        "vec3<f32>(sin(a), 0.5, shader_time())".to_owned(),
    );
    own.settings.insert("gives".to_owned(), "vec3".to_owned());
    let made = graph(vec![node("time", "time", &[]), own], &[("emissive", "own")]);
    let code = graph_code(&made).expect("it's right");
    check_surface_code(&code).unwrap_or_else(|problem| panic!("{problem}\n{code}"));
}

#[test]
fn what_is_wrong_is_said_by_node() {
    let cases: [(ShaderGraph, &str); 9] = [
        (
            graph(vec![node("a", "wobble", &[])], &[("base", "a")]),
            "a: there's no kind called wobble",
        ),
        (
            graph(vec![node("a", "sin", &[("a", "gone")])], &[("base", "a")]),
            "a reads gone, which isn't there",
        ),
        (
            graph(
                vec![
                    node("a", "sin", &[("a", "b")]),
                    node("b", "sin", &[("a", "a")]),
                ],
                &[("base", "a")],
            ),
            "goes round in a loop",
        ),
        (
            graph(vec![node("a", "uv", &[])], &[("vertex", "a")]),
            "a: uv can't be read in vertex",
        ),
        (
            graph(vec![node("a", "color", &[])], &[("base", "a")]),
            "a: color can't be read in surface",
        ),
        (
            graph(vec![node("a", "time", &[])], &[("wobble", "a")]),
            "there's no output called wobble",
        ),
        (
            graph(vec![node("a", "time", &[])], &[]),
            "nothing goes to an output",
        ),
        (
            graph(
                vec![node("a", "time", &[]), node("a", "uv", &[])],
                &[("base", "a")],
            ),
            "two nodes called a",
        ),
        (
            graph(vec![node("a", "dot", &[])], &[("base", "a")]),
            "a: dot needs more than a number",
        ),
    ];
    for (made, said) in cases {
        let told = problems(&made);
        assert!(told.contains(said), "{said}: {told}");
    }
}

#[test]
fn a_node_used_twice_is_written_once() {
    let made = graph(
        vec![
            node("time", "time", &[]),
            node("wave", "sin", &[("a", "time")]),
            node("both", "add", &[("a", "wave"), ("b", "wave")]),
        ],
        &[("emissive", "both")],
    );
    let code = graph_code(&made).expect("it's right");
    assert_eq!(code.matches("sin(").count(), 1, "{code}");
}

#[test]
fn an_empty_map_of_inputs_takes_defaults() {
    let made = graph(vec![node("mix", "mix", &[])], &[("emissive", "mix")]);
    let code = graph_code(&made).expect("it's right");
    assert!(code.contains("mix(0.0f, 1.0f, 0.5f)"), "{code}");
    let inputs: BTreeMap<String, GraphInput> = BTreeMap::new();
    assert!(made.nodes[0].inputs == inputs);
}

/// A folder with `files` in `shaders/`.
fn shaders(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("engine-graph-{name}-{}", std::process::id()));
    let _gone = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("shaders")).expect("folder");
    for (file, text) in files {
        std::fs::write(root.join("shaders").join(file), text).expect("write");
    }
    root
}

/// What a material gets for the shader called `name` in `root`.
fn shader_in(root: &std::path::Path, name: &str) -> String {
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(DirectoryArchive::new(root)));
    MaterialLibrary::new(&filesystem).shader(name).code.clone()
}

const WAVE: &str = "format 1 graph
node { id = time kind = time }
node { id = wave kind = sin a = time }
outputs { emissive = wave }
";

#[test]
fn a_material_finds_a_graph_as_it_does_a_shader() {
    let root = shaders("found", &[("wave.egraph", WAVE)]);
    let code = shader_in(&root, "wave");
    assert!(code.contains("fn shader_surface"), "{code}");
    let _gone = std::fs::remove_dir_all(root);
}

#[test]
fn code_of_the_same_name_goes_ahead_of_a_graph() {
    let by_hand = "fn shader_uv(uv: vec2<f32>, world: vec3<f32>) -> vec2<f32> { return uv; }\n";
    let root = shaders("both", &[("wave.egraph", WAVE), ("wave.wgsl", by_hand)]);
    assert_eq!(shader_in(&root, "wave"), by_hand);
    let _gone = std::fs::remove_dir_all(root);
}

#[test]
fn a_wrong_graph_is_a_shader_that_wont_compile_and_says_why() {
    let wrong = WAVE.replace("a = time", "a = nothing");
    let root = shaders("wrong", &[("wave.egraph", &wrong)]);
    let code = shader_in(&root, "wave");
    assert!(code.contains("wave reads nothing"), "{code}");
    let told = check_surface_code(&code).expect_err("it's broken");
    assert!(!told.is_empty());
    let _gone = std::fs::remove_dir_all(root);
}

#[test]
fn a_graph_can_be_a_fields_step() {
    let made = graph(
        vec![
            node("last", "last", &[]),
            node("left", "last", &[]),
            node("cell", "cell", &[]),
            node("uv", "uv", &[]),
            node("time", "time", &[]),
            node("noise", "noise", &[]),
            node("sum", "add", &[("a", "last"), ("b", "left")]),
            node("more", "add", &[("a", "sum"), ("b", "cell")]),
            node("again", "add", &[("a", "more"), ("b", "uv")]),
            node("all", "multiply", &[("a", "again"), ("b", "noise")]),
        ],
        &[("field", "all")],
    );
    let code = graph_code(&made).expect("it's right");
    assert!(code.contains("fn shader_field"), "{code}");
    check_field_code(&code).unwrap_or_else(|problem| panic!("{problem}\n{code}"));
}

#[test]
fn a_fields_step_goes_alone_and_only_it_reads_cells() {
    let both = graph(
        vec![node("last", "last", &[]), node("time", "time", &[])],
        &[("field", "last"), ("base", "time")],
    );
    assert!(problems(&both).contains("field is for a field's step"));
    let misplaced = graph(vec![node("last", "last", &[])], &[("base", "last")]);
    assert!(problems(&misplaced).contains("last can't be read in surface"));
}

#[test]
fn a_graph_can_be_what_a_particle_does() {
    let made = graph(
        vec![
            node("tinted", "tinted", &[]),
            node("life", "life", &[]),
            node("noise", "noise", &[]),
            node("fade", "one_minus", &[("a", "life")]),
            node("both", "multiply", &[("a", "fade"), ("b", "noise")]),
            node("all", "multiply", &[("a", "tinted"), ("b", "both")]),
        ],
        &[("particle", "all")],
    );
    let code = graph_code(&made).expect("it's right");
    check_particle_code(&code).unwrap_or_else(|problem| panic!("{problem}\n{code}"));
    let misplaced = graph(vec![node("life", "life", &[])], &[("base", "life")]);
    assert!(problems(&misplaced).contains("life can't be read in surface"));
}
