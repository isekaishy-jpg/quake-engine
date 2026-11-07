//! Changing a shader graph, and what the node view shows of it (proposal 0145).

use engine_compile::{GRAPH_OUTPUTS, GraphInput, GraphNode, ShaderGraph, graph_kind};

use crate::graph_view::{ViewEvent, ViewNode, ViewWire};

/// The id of the box that stands for the outputs, which isn't a node of the graph.
pub const OUTPUTS: &str = "outputs";

/// Adds a node of `kind` at `at`, and says what it's called. A kind that's not there adds nothing.
pub fn add_node(graph: &mut ShaderGraph, kind: &str, at: [f32; 2]) -> Option<String> {
    graph_kind(kind)?;
    let taken = |id: &str| graph.nodes.iter().any(|node| node.id == id);
    let id = (1..)
        .map(|number| format!("{kind}{number}"))
        .find(|id| !taken(id))?;
    graph.nodes.push(GraphNode {
        id: id.clone(),
        kind: kind.to_owned(),
        at,
        ..GraphNode::default()
    });
    Some(id)
}

/// Takes node `id` away, and what was joined to it.
pub fn remove_node(graph: &mut ShaderGraph, id: &str) {
    graph.nodes.retain(|node| node.id != id);
    for node in &mut graph.nodes {
        node.inputs
            .retain(|_, input| *input != GraphInput::Node(id.to_owned()));
    }
    graph.outputs.retain(|_, node| node != id);
}

/// Joins `from` to `input` of `to`, which can be the outputs. A join that would make a loop isn't
/// made, and says so.
pub fn join(graph: &mut ShaderGraph, from: &str, to: &str, input: &str) -> bool {
    if to == OUTPUTS {
        graph.outputs.insert(input.to_owned(), from.to_owned());
        return true;
    }
    if from == to || reaches(graph, from, to) {
        return false;
    }
    let found = graph.nodes.iter_mut().find(|node| node.id == to);
    if let Some(node) = found {
        node.inputs
            .insert(input.to_owned(), GraphInput::Node(from.to_owned()));
    }
    true
}

/// Whether `id` reads `what`, however far back.
fn reaches(graph: &ShaderGraph, id: &str, what: &str) -> bool {
    let Some(node) = graph.nodes.iter().find(|node| node.id == id) else {
        return false;
    };
    node.inputs.values().any(|input| match input {
        GraphInput::Node(other) => other == what || reaches(graph, other, what),
        GraphInput::Number(_) => false,
    })
}

/// Takes what's joined to `input` of `to` away.
pub fn cut(graph: &mut ShaderGraph, to: &str, input: &str) {
    if to == OUTPUTS {
        graph.outputs.remove(input);
    } else if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == to) {
        node.inputs.remove(input);
    }
}

/// Where the outputs' box goes: to the right of everything.
fn outputs_at(graph: &ShaderGraph) -> [f32; 2] {
    let right = graph
        .nodes
        .iter()
        .map(|node| node.at[0])
        .fold(0.0, f32::max);
    [right + 240.0, 20.0]
}

/// What the node view shows of `graph`. Its boxes go in the graph's order, with the outputs last.
#[must_use]
pub fn view_of(graph: &ShaderGraph) -> (Vec<ViewNode>, Vec<ViewWire>) {
    let mut boxes: Vec<ViewNode> = graph
        .nodes
        .iter()
        .map(|node| ViewNode {
            title: node.id.clone(),
            at: node.at,
            inputs: graph_kind(&node.kind).map_or_else(Vec::new, |kind| {
                kind.inputs
                    .iter()
                    .map(|(name, _)| (*name).to_owned())
                    .collect()
            }),
            output: true,
        })
        .collect();
    let outputs = boxes.len();
    boxes.push(ViewNode {
        title: OUTPUTS.to_owned(),
        at: outputs_at(graph),
        inputs: GRAPH_OUTPUTS
            .iter()
            .map(|(name, ..)| (*name).to_owned())
            .collect(),
        output: false,
    });
    let number_of = |id: &str| graph.nodes.iter().position(|node| node.id == id);
    let mut wires = Vec::new();
    for (to, node) in graph.nodes.iter().enumerate() {
        let Some(kind) = graph_kind(&node.kind) else {
            continue;
        };
        for (input, (name, _)) in kind.inputs.iter().enumerate() {
            if let Some(GraphInput::Node(other)) = node.inputs.get(*name)
                && let Some(from) = number_of(other)
            {
                wires.push(ViewWire { from, to, input });
            }
        }
    }
    for (input, (name, ..)) in GRAPH_OUTPUTS.iter().enumerate() {
        if let Some(from) = graph.outputs.get(*name).and_then(|id| number_of(id)) {
            wires.push(ViewWire {
                from,
                to: outputs,
                input,
            });
        }
    }
    (boxes, wires)
}

/// What box `number` is, and the name of input `input` of it.
fn input_named(graph: &ShaderGraph, number: usize, input: usize) -> Option<(String, String)> {
    if number == graph.nodes.len() {
        let (name, ..) = GRAPH_OUTPUTS.get(input)?;
        return Some((OUTPUTS.to_owned(), (*name).to_owned()));
    }
    let node = graph.nodes.get(number)?;
    let (name, _) = graph_kind(&node.kind)?.inputs.get(input)?;
    Some((node.id.clone(), (*name).to_owned()))
}

/// Does what a person did in the view. Says if the graph changed, so that it can be gone back on.
pub fn apply(graph: &mut ShaderGraph, event: ViewEvent) -> bool {
    match event {
        ViewEvent::Chosen(_) => false,
        ViewEvent::Moved(number, at) => {
            if let Some(node) = graph.nodes.get_mut(number) {
                node.at = at;
            }
            false
        }
        ViewEvent::Joined(wire) => {
            let from = graph.nodes.get(wire.from).map(|node| node.id.clone());
            let to = input_named(graph, wire.to, wire.input);
            match (from, to) {
                (Some(from), Some((to, input))) => join(graph, &from, &to, &input),
                _ => false,
            }
        }
        ViewEvent::Cut(number, input) => {
            let Some((to, input)) = input_named(graph, number, input) else {
                return false;
            };
            cut(graph, &to, &input);
            true
        }
    }
}
