//! Changing a shader graph, and finding things in the node view (proposal 0145).

use engine_compile::{GraphInput, ShaderGraph, graph_code};
use engine_editor::graph_edit::{OUTPUTS, add_node, apply, cut, join, remove_node, view_of};
use engine_editor::graph_view::{
    ViewEvent, ViewNode, ViewWire, input_at, input_port, node_at, output_at, output_port,
};

/// Time through a sine into a surface's light.
fn wave() -> ShaderGraph {
    let mut graph = ShaderGraph::default();
    let time = add_node(&mut graph, "time", [0.0, 0.0]).expect("a kind");
    let wave = add_node(&mut graph, "sin", [200.0, 0.0]).expect("a kind");
    assert!(join(&mut graph, &time, &wave, "a"));
    assert!(join(&mut graph, &wave, OUTPUTS, "emissive"));
    graph
}

#[test]
fn a_graph_made_a_node_at_a_time_comes_to_code() {
    let code = graph_code(&wave()).expect("it's right");
    assert!(code.contains("sin("), "{code}");
}

#[test]
fn nodes_get_names_of_their_own() {
    let mut graph = ShaderGraph::default();
    assert_eq!(
        add_node(&mut graph, "sin", [0.0; 2]).as_deref(),
        Some("sin1")
    );
    assert_eq!(
        add_node(&mut graph, "sin", [0.0; 2]).as_deref(),
        Some("sin2")
    );
    remove_node(&mut graph, "sin1");
    assert_eq!(
        add_node(&mut graph, "sin", [0.0; 2]).as_deref(),
        Some("sin1")
    );
    assert_eq!(add_node(&mut graph, "wobble", [0.0; 2]), None);
}

#[test]
fn a_join_that_would_make_a_loop_isnt_made() {
    let mut graph = wave();
    assert!(
        !join(&mut graph, "sin1", "time1", "a"),
        "there's no input, and it'd loop"
    );
    assert!(!join(&mut graph, "sin1", "sin1", "a"), "to itself");
    let add = add_node(&mut graph, "add", [0.0; 2]).expect("a kind");
    assert!(join(&mut graph, "sin1", &add, "a"));
    assert!(
        !join(&mut graph, &add, "sin1", "a"),
        "round through another"
    );
    assert!(graph_code(&graph).is_ok());
}

#[test]
fn taking_a_node_away_takes_what_joined_it() {
    let mut graph = wave();
    remove_node(&mut graph, "time1");
    assert!(graph.nodes[0].inputs.is_empty());
    remove_node(&mut graph, "sin1");
    assert!(graph.outputs.is_empty());
}

#[test]
fn a_join_can_be_cut() {
    let mut graph = wave();
    cut(&mut graph, "sin1", "a");
    assert!(graph.nodes[1].inputs.is_empty());
    cut(&mut graph, OUTPUTS, "emissive");
    assert!(graph.outputs.is_empty());
}

#[test]
fn the_view_shows_every_join_with_the_outputs_last() {
    let graph = wave();
    let (boxes, wires) = view_of(&graph);
    assert_eq!(boxes.len(), 3);
    assert_eq!(boxes[2].title, OUTPUTS);
    assert!(!boxes[2].output);
    let emissive = boxes[2]
        .inputs
        .iter()
        .position(|name| name == "emissive")
        .expect("an input");
    assert_eq!(
        wires,
        [
            ViewWire {
                from: 0,
                to: 1,
                input: 0
            },
            ViewWire {
                from: 1,
                to: 2,
                input: emissive
            },
        ]
    );
}

#[test]
fn what_is_done_in_the_view_is_done_to_the_graph() {
    let mut graph = wave();
    let (boxes, _) = view_of(&graph);
    let base = boxes[2]
        .inputs
        .iter()
        .position(|name| name == "base")
        .expect("an input");
    let joined = ViewEvent::Joined(ViewWire {
        from: 1,
        to: 2,
        input: base,
    });
    assert!(apply(&mut graph, joined));
    assert_eq!(graph.outputs.get("base").map(String::as_str), Some("sin1"));
    assert!(apply(&mut graph, ViewEvent::Cut(1, 0)));
    assert!(!graph.nodes[1].inputs.contains_key("a"));
    assert!(!apply(&mut graph, ViewEvent::Moved(0, [50.0, 60.0])));
    assert_eq!(graph.nodes[0].at, [50.0, 60.0]);
    let looping = ViewEvent::Joined(ViewWire {
        from: 1,
        to: 1,
        input: 0,
    });
    assert!(!apply(&mut graph, looping));
    assert!(graph.nodes[1].inputs.get("a") != Some(&GraphInput::Node("sin1".to_owned())));
}

#[test]
fn ports_and_boxes_are_found_where_they_are() {
    let nodes = [
        ViewNode {
            title: "a".to_owned(),
            at: [0.0, 0.0],
            inputs: vec!["x".to_owned(), "y".to_owned()],
            output: true,
        },
        ViewNode {
            title: "b".to_owned(),
            at: [300.0, 100.0],
            inputs: vec!["x".to_owned()],
            output: false,
        },
    ];
    assert_eq!(input_at(&nodes, input_port(&nodes[0], 1)), Some((0, 1)));
    assert_eq!(input_at(&nodes, input_port(&nodes[1], 0)), Some((1, 0)));
    assert_eq!(output_at(&nodes, output_port(&nodes[0])), Some(0));
    assert_eq!(
        output_at(&nodes, output_port(&nodes[1])),
        None,
        "it has none"
    );
    assert_eq!(node_at(&nodes, [320.0, 110.0]), Some(1));
    assert_eq!(node_at(&nodes, [200.0, 200.0]), None);
    assert_eq!(input_at(&nodes, [200.0, 200.0]), None);
}
