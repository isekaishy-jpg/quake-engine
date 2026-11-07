//! Shader graphs (proposal 0145): nodes in a file, which come out as surface code. Past that
//! they're a shader like any other, so nothing that draws knows about them.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use engine_format::{Block, Document, Value, from_text, to_text};

/// How wide a value is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GraphType {
    /// A number.
    One,
    /// Two of them.
    Two,
    /// Three of them.
    Three,
    /// Four of them.
    Four,
}

impl GraphType {
    /// Its name in WGSL.
    const fn wgsl(self) -> &'static str {
        match self {
            Self::One => "f32",
            Self::Two => "vec2<f32>",
            Self::Three => "vec3<f32>",
            Self::Four => "vec4<f32>",
        }
    }

    /// The type a graph calls `name`.
    fn called(name: &str) -> Option<Self> {
        match name {
            "number" => Some(Self::One),
            "vec2" => Some(Self::Two),
            "vec3" => Some(Self::Three),
            "vec4" => Some(Self::Four),
            _ => None,
        }
    }
}

/// `code` of type `from`, as a `to`. A number fills what it goes into, more goes narrower by
/// dropping its end, and what has to grow gets nothing but a whole alpha.
fn cast(code: &str, from: GraphType, to: GraphType) -> String {
    use GraphType::{Four, One, Three, Two};
    match (from, to) {
        (a, b) if a == b => code.to_owned(),
        (One, wide) => format!("{}({code})", wide.wgsl()),
        (Two, One) | (Three, One) | (Four, One) => format!("({code}).x"),
        (Three, Two) | (Four, Two) => format!("({code}).xy"),
        (Four, Three) => format!("({code}).xyz"),
        (Two, Three) => format!("vec3<f32>({code}, 0.0)"),
        (Two, Four) => format!("vec4<f32>({code}, 0.0, 1.0)"),
        (Three, Four) => format!("vec4<f32>({code}, 1.0)"),
        _ => code.to_owned(),
    }
}

/// What part of a shader a value can be read in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphHook {
    /// Where a surface's corners go.
    Vertex,
    /// Where it reads its pictures.
    Uv,
    /// What it is, ahead of its light.
    Surface,
    /// The level's final pass.
    Post,
}

impl GraphHook {
    /// Its name, to say what's wrong.
    const fn name(self) -> &'static str {
        match self {
            Self::Vertex => "vertex",
            Self::Uv => "uv",
            Self::Surface => "surface",
            Self::Post => "post",
        }
    }
}

/// How a kind works out what it gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gives {
    /// Always this.
    Fixed(GraphType),
    /// As wide as the widest of what it's given, which all go to that.
    Widest,
    /// What its first input is, with the rest numbers.
    First,
    /// What it's told.
    Told,
}

/// A kind of node.
#[derive(Clone, Copy, Debug)]
pub struct GraphKind {
    /// Its name.
    pub name: &'static str,
    /// Which group it's listed under.
    pub group: &'static str,
    /// Its inputs, and what each is if nothing's joined to it.
    pub inputs: &'static [(&'static str, f32)],
    /// What else it's told, as values on the node.
    pub settings: &'static [&'static str],
    /// How wide what it gives is.
    gives: Gives,
    /// The WGSL, with `{a}` and so on for its inputs, and `{value}` for a setting.
    code: &'static str,
}

/// Input names.
const A: &[(&str, f32)] = &[("a", 0.0)];
/// Two inputs.
const AB: &[(&str, f32)] = &[("a", 0.0), ("b", 0.0)];
/// Nothing.
const NONE: &[(&str, f32)] = &[];

/// A kind that's a function of what it's given, in the sums group.
const fn sum(
    name: &'static str,
    inputs: &'static [(&'static str, f32)],
    code: &'static str,
) -> GraphKind {
    GraphKind {
        name,
        group: "sums",
        inputs,
        settings: &[],
        gives: Gives::Widest,
        code,
    }
}

/// A kind that's there to be read, in the given group.
const fn read(name: &'static str, gives: GraphType, code: &'static str) -> GraphKind {
    GraphKind {
        name,
        group: "there",
        inputs: NONE,
        settings: &[],
        gives: Gives::Fixed(gives),
        code,
    }
}

/// Every kind of node.
pub const GRAPH_KINDS: [GraphKind; 34] = [
    read("uv", GraphType::Two, "uv"),
    read("world", GraphType::Three, "world"),
    read("normal", GraphType::Three, "surface.normal"),
    read("base", GraphType::Four, "surface.base"),
    read("color", GraphType::Three, "color"),
    read("time", GraphType::One, "shader_time()"),
    GraphKind {
        name: "param",
        group: "there",
        inputs: NONE,
        settings: &["index"],
        gives: Gives::Fixed(GraphType::One),
        code: "shader_param({index})",
    },
    GraphKind {
        name: "image",
        group: "there",
        inputs: &[("at", 0.0)],
        settings: &["index"],
        gives: Gives::Fixed(GraphType::Four),
        code: "shader_image({index}, {at})",
    },
    GraphKind {
        name: "number",
        group: "values",
        inputs: NONE,
        settings: &["value"],
        gives: Gives::Fixed(GraphType::One),
        code: "{value}",
    },
    GraphKind {
        name: "join",
        group: "values",
        inputs: &[("x", 0.0), ("y", 0.0), ("z", 0.0), ("w", 1.0)],
        settings: &[],
        gives: Gives::Fixed(GraphType::Four),
        code: "vec4<f32>({x}, {y}, {z}, {w})",
    },
    GraphKind {
        name: "split",
        group: "values",
        inputs: &[("a", 0.0)],
        settings: &["part"],
        gives: Gives::Fixed(GraphType::One),
        code: "({a}).{part}",
    },
    sum("add", AB, "({a} + {b})"),
    sum("subtract", AB, "({a} - {b})"),
    sum("multiply", &[("a", 1.0), ("b", 1.0)], "({a} * {b})"),
    sum("divide", &[("a", 0.0), ("b", 1.0)], "({a} / {b})"),
    sum("power", &[("a", 0.0), ("b", 1.0)], "pow({a}, {b})"),
    sum("min", AB, "min({a}, {b})"),
    sum("max", AB, "max({a}, {b})"),
    sum(
        "clamp",
        &[("a", 0.0), ("low", 0.0), ("high", 1.0)],
        "clamp({a}, {low}, {high})",
    ),
    sum("step", &[("edge", 0.5), ("a", 0.0)], "step({edge}, {a})"),
    sum(
        "smoothstep",
        &[("low", 0.0), ("high", 1.0), ("a", 0.0)],
        "smoothstep({low}, {high}, {a})",
    ),
    GraphKind {
        name: "mix",
        group: "sums",
        inputs: &[("a", 0.0), ("b", 1.0), ("share", 0.5)],
        settings: &[],
        gives: Gives::Widest,
        code: "mix({a}, {b}, {share})",
    },
    sum("sin", A, "sin({a})"),
    sum("cos", A, "cos({a})"),
    sum("abs", A, "abs({a})"),
    sum("fract", A, "fract({a})"),
    sum("floor", A, "floor({a})"),
    sum("one_minus", A, "(1.0 - {a})"),
    GraphKind {
        name: "normalize",
        group: "sums",
        inputs: A,
        settings: &[],
        gives: Gives::First,
        code: "normalize({a})",
    },
    GraphKind {
        name: "length",
        group: "sums",
        inputs: A,
        settings: &[],
        gives: Gives::Fixed(GraphType::One),
        code: "length({a})",
    },
    GraphKind {
        name: "dot",
        group: "sums",
        inputs: AB,
        settings: &[],
        gives: Gives::Fixed(GraphType::One),
        code: "dot({a}, {b})",
    },
    GraphKind {
        name: "scale",
        group: "sums",
        inputs: &[("a", 0.0), ("by", 1.0)],
        settings: &[],
        gives: Gives::First,
        code: "({a} * {by})",
    },
    GraphKind {
        name: "noise",
        group: "sums",
        inputs: &[("at", 0.0)],
        settings: &[],
        gives: Gives::Fixed(GraphType::One),
        code: "shader_noise({at})",
    },
    GraphKind {
        name: "code",
        group: "code",
        inputs: &[("a", 0.0), ("b", 0.0), ("c", 0.0), ("d", 0.0)],
        settings: &["code", "gives"],
        gives: Gives::Told,
        code: "",
    },
];

/// The outputs of a graph: the name, its hook, and how wide it has to be.
pub const GRAPH_OUTPUTS: [(&str, GraphHook, GraphType); 6] = [
    ("vertex", GraphHook::Vertex, GraphType::Three),
    ("uv", GraphHook::Uv, GraphType::Two),
    ("base", GraphHook::Surface, GraphType::Four),
    ("normal", GraphHook::Surface, GraphType::Three),
    ("emissive", GraphHook::Surface, GraphType::Three),
    ("color", GraphHook::Post, GraphType::Three),
];

/// The kind called `name`.
#[must_use]
pub fn graph_kind(name: &str) -> Option<&'static GraphKind> {
    GRAPH_KINDS.iter().find(|kind| kind.name == name)
}

/// Whether a kind can be read in `hook`.
fn readable(kind: &str, hook: GraphHook) -> bool {
    use GraphHook::{Post, Surface, Uv, Vertex};
    match kind {
        "uv" => matches!(hook, Uv | Post | Surface),
        "world" => matches!(hook, Vertex | Uv | Surface),
        "normal" | "base" => hook == Surface,
        "color" => hook == Post,
        "time" | "param" => hook != Post,
        "image" => matches!(hook, Uv | Surface),
        "noise" => hook != Post,
        _ => true,
    }
}

/// What's put into a node's input.
#[derive(Clone, Debug, PartialEq)]
pub enum GraphInput {
    /// A number.
    Number(f32),
    /// What another node gives, by its id.
    Node(String),
}

/// One node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GraphNode {
    /// What it's known by.
    pub id: String,
    /// Its kind.
    pub kind: String,
    /// Where it sits in the editor.
    pub at: [f32; 2],
    /// What's joined to its inputs. An input that isn't here has its default.
    pub inputs: BTreeMap<String, GraphInput>,
    /// What else it's told.
    pub settings: BTreeMap<String, String>,
}

/// A shader as a graph.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShaderGraph {
    /// Its nodes.
    pub nodes: Vec<GraphNode>,
    /// Which node goes to each output.
    pub outputs: BTreeMap<String, String>,
}

/// The graph in a file.
///
/// # Errors
///
/// Says what's wrong if it can't be read.
pub fn read_graph(text: &str) -> Result<ShaderGraph, String> {
    let document = from_text(text).map_err(|error| error.to_string())?;
    if document.kind != "graph" {
        return Err("that isn't a graph".to_owned());
    }
    let mut graph = ShaderGraph::default();
    for block in document.blocks_of("node") {
        graph.nodes.push(read_node(block)?);
    }
    for block in document.blocks_of("outputs") {
        for (name, value) in &block.fields {
            if let Some(node) = value.text() {
                graph.outputs.insert(name.clone(), node.to_owned());
            }
        }
    }
    Ok(graph)
}

/// A node from its block.
#[expect(clippy::cast_possible_truncation, reason = "shaders are in f32")]
fn read_node(block: &Block) -> Result<GraphNode, String> {
    let id = block.get("id").and_then(Value::text);
    let id = id.ok_or("a node needs an id")?;
    let kind = block.get("kind").and_then(Value::text);
    let kind = kind.ok_or_else(|| format!("{id} needs a kind"))?;
    let at = block.get("at").and_then(Value::numbers).unwrap_or_default();
    let mut node = GraphNode {
        id: id.to_owned(),
        kind: kind.to_owned(),
        at: [
            at.first().copied().unwrap_or(0.0) as f32,
            at.get(1).copied().unwrap_or(0.0) as f32,
        ],
        ..GraphNode::default()
    };
    let known = graph_kind(kind);
    let setting = |name: &str| known.is_some_and(|kind| kind.settings.contains(&name));
    for (name, value) in &block.fields {
        if matches!(name.as_str(), "id" | "kind" | "at") {
            continue;
        }
        if setting(name) {
            let text = match value {
                Value::Number(number) => number.to_string(),
                other => other.text().unwrap_or_default().to_owned(),
            };
            node.settings.insert(name.clone(), text);
        } else if let Some(number) = value.number() {
            node.inputs
                .insert(name.clone(), GraphInput::Number(number as f32));
        } else if let Some(other) = value.text() {
            node.inputs
                .insert(name.clone(), GraphInput::Node(other.to_owned()));
        }
    }
    Ok(node)
}

/// `graph` as a file.
#[must_use]
pub fn write_graph(graph: &ShaderGraph) -> String {
    let mut document = Document::new("graph");
    for node in &graph.nodes {
        let mut block = Block::new("node");
        block.set("id", Value::Text(node.id.clone()));
        block.set("kind", Value::Text(node.kind.clone()));
        let at = node.at.map(|part| Value::Number(f64::from(part)));
        block.set("at", Value::List(at.to_vec()));
        for (name, input) in &node.inputs {
            let value = match input {
                GraphInput::Number(number) => Value::Number(f64::from(*number)),
                GraphInput::Node(other) => Value::Text(other.clone()),
            };
            block.set(name, value);
        }
        for (name, value) in &node.settings {
            block.set(name, Value::Text(value.clone()));
        }
        document.blocks.push(block);
    }
    let mut outputs = Block::new("outputs");
    for (name, node) in &graph.outputs {
        outputs.set(name, Value::Text(node.clone()));
    }
    document.blocks.push(outputs);
    to_text(&document)
}

/// A number as WGSL, with its type, as some built-ins won't take one that's yet to have one.
fn float(value: f32) -> String {
    format!("{value:?}f")
}

/// Writes one hook of a graph: a line for each node it needs, each after what it reads.
struct Writer<'a> {
    /// The graph's nodes, by id.
    nodes: HashMap<&'a str, (usize, &'a GraphNode)>,
    /// Which hook this is.
    hook: GraphHook,
    /// What's been written, with its name and type.
    done: HashMap<&'a str, (String, GraphType)>,
    /// What's being written, to find a loop.
    open: Vec<&'a str>,
    /// The lines.
    lines: String,
    /// Functions for `code` nodes, which go ahead of the hook.
    helpers: String,
    /// What's wrong.
    problems: Vec<String>,
}

impl<'a> Writer<'a> {
    /// The name and type of what node `id` gives, writing it if it's not written.
    fn value(&mut self, id: &'a str, reader: &str) -> Option<(String, GraphType)> {
        if let Some(found) = self.done.get(id) {
            return Some(found.clone());
        }
        if self.open.contains(&id) {
            self.problems.push(format!("{id} goes round in a loop"));
            return None;
        }
        let Some(&(number, node)) = self.nodes.get(id) else {
            self.problems
                .push(format!("{reader} reads {id}, which isn't there"));
            return None;
        };
        let Some(kind) = graph_kind(&node.kind) else {
            self.problems
                .push(format!("{id}: there's no kind called {}", node.kind));
            return None;
        };
        if !readable(kind.name, self.hook) {
            self.problems.push(format!(
                "{id}: {} can't be read in {}",
                kind.name,
                self.hook.name()
            ));
            return None;
        }
        self.open.push(id);
        let written = self.write(number, node, kind);
        self.open.pop();
        let (code, gives) = written?;
        let name = format!("n{number}");
        let _ = writeln!(self.lines, "    let {name}: {} = {code};", gives.wgsl());
        self.done.insert(id, (name.clone(), gives));
        Some((name, gives))
    }

    /// What goes into `input` of `node`, with the type it comes as.
    fn input(
        &mut self,
        node: &'a GraphNode,
        input: &str,
        default: f32,
    ) -> Option<(String, GraphType)> {
        match node.inputs.get(input) {
            Some(GraphInput::Node(other)) => self.value(other, &node.id),
            Some(GraphInput::Number(number)) => Some((float(*number), GraphType::One)),
            // A place left out is where this pixel is.
            None if input == "at" && self.hook == GraphHook::Vertex => {
                Some(("world.xy".to_owned(), GraphType::Two))
            }
            None if input == "at" => Some(("uv".to_owned(), GraphType::Two)),
            None => Some((float(default), GraphType::One)),
        }
    }

    /// The code of `node`, and what it gives.
    fn write(
        &mut self,
        number: usize,
        node: &'a GraphNode,
        kind: &GraphKind,
    ) -> Option<(String, GraphType)> {
        let mut given = Vec::with_capacity(kind.inputs.len());
        for (name, default) in kind.inputs {
            given.push((*name, self.input(node, name, *default)?));
        }
        if kind.name == "code" {
            return self.code_node(number, node, &given);
        }
        if matches!(kind.name, "normalize" | "dot") && widest_of(&given) == GraphType::One {
            self.problems.push(format!(
                "{}: {} needs more than a number",
                node.id, kind.name
            ));
            return None;
        }
        let gives = match kind.gives {
            Gives::Fixed(gives) => gives,
            Gives::Widest => given
                .iter()
                .map(|(_, (_, gives))| *gives)
                .max()
                .unwrap_or(GraphType::One),
            Gives::First => given
                .first()
                .map_or(GraphType::One, |(_, (_, gives))| *gives),
            Gives::Told => GraphType::One,
        };
        let mut code = kind.code.to_owned();
        for (at, (name, (value, from))) in given.iter().enumerate() {
            let to = match (kind.gives, *name, kind.name) {
                (_, "at", "image" | "noise") => GraphType::Two,
                (Gives::Widest, _, _) => gives,
                (Gives::First, _, _) if at > 0 => GraphType::One,
                _ if kind.name == "dot" => widest_of(&given),
                _ if kind.name == "join" => GraphType::One,
                _ => *from,
            };
            code = code.replace(&format!("{{{name}}}"), &cast(value, *from, to));
        }
        for setting in kind.settings {
            let value = self.setting(node, setting)?;
            code = code.replace(&format!("{{{setting}}}"), &value);
        }
        Some((code, gives))
    }

    /// A node's setting, as it goes into code.
    fn setting(&mut self, node: &GraphNode, name: &str) -> Option<String> {
        let found = node.settings.get(name).map(String::as_str);
        match name {
            "index" => {
                let index = found.unwrap_or("0");
                if index.parse::<u8>().is_ok() {
                    return Some(index.to_owned());
                }
                self.problems
                    .push(format!("{}: index has to be a whole number", node.id));
                None
            }
            "value" => {
                let parsed = found.unwrap_or("0").parse::<f32>();
                let Ok(value) = parsed else {
                    self.problems
                        .push(format!("{}: value has to be a number", node.id));
                    return None;
                };
                Some(float(value))
            }
            "part" => {
                let part = found.unwrap_or("x");
                if matches!(part, "x" | "y" | "z" | "w") {
                    return Some(part.to_owned());
                }
                self.problems
                    .push(format!("{}: part has to be x, y, z or w", node.id));
                None
            }
            _ => found.map(str::to_owned),
        }
    }

    /// A `code` node, which is a function of its own, so that its inputs can have their names.
    fn code_node(
        &mut self,
        number: usize,
        node: &GraphNode,
        given: &[(&str, (String, GraphType))],
    ) -> Option<(String, GraphType)> {
        let Some(gives) = node
            .settings
            .get("gives")
            .and_then(|name| GraphType::called(name))
        else {
            self.problems.push(format!(
                "{}: gives has to be number, vec2, vec3 or vec4",
                node.id
            ));
            return None;
        };
        let code = node.settings.get("code").map_or("", String::as_str);
        if code.trim().is_empty() {
            self.problems.push(format!("{}: there's no code", node.id));
            return None;
        }
        let parameters: Vec<String> = given
            .iter()
            .map(|(name, (_, from))| format!("{name}: {}", from.wgsl()))
            .collect();
        let name = format!("graph_code_{number}");
        if !self.helpers.contains(&format!("fn {name}(")) {
            let _ = writeln!(
                self.helpers,
                "fn {name}({}) -> {} {{\n    return {code};\n}}\n",
                parameters.join(", "),
                gives.wgsl()
            );
        }
        let arguments: Vec<&str> = given.iter().map(|(_, (value, _))| value.as_str()).collect();
        Some((format!("{name}({})", arguments.join(", ")), gives))
    }
}

/// The widest of what's `given`.
fn widest_of(given: &[(&str, (String, GraphType))]) -> GraphType {
    let widths = given.iter().map(|(_, (_, gives))| *gives);
    widths.max().unwrap_or(GraphType::One)
}

/// What each output of a hook comes to.
type HookResults = Vec<(&'static str, String)>;

/// What goes into a hook: the functions ahead of it, its lines, and what they come to.
struct HookCode {
    /// Functions for `code` nodes.
    helpers: String,
    /// A line for each node.
    lines: String,
    /// What each output comes to, in the type it has to be.
    results: HookResults,
}

/// What `graph` needs in `hook`, if it wants anything there.
fn hook_lines(
    graph: &ShaderGraph,
    hook: GraphHook,
    problems: &mut Vec<String>,
) -> Option<HookCode> {
    let wanted: Vec<_> = GRAPH_OUTPUTS
        .iter()
        .filter(|(name, at, _)| *at == hook && graph.outputs.contains_key(*name))
        .collect();
    if wanted.is_empty() {
        return None;
    }
    let mut writer = Writer {
        nodes: (0..)
            .zip(&graph.nodes)
            .map(|(at, node)| (node.id.as_str(), (at, node)))
            .collect(),
        hook,
        done: HashMap::new(),
        open: Vec::new(),
        lines: String::new(),
        helpers: String::new(),
        problems: Vec::new(),
    };
    let mut results = Vec::with_capacity(wanted.len());
    for (name, _, to) in wanted {
        let id = graph.outputs.get(*name).map_or("", String::as_str);
        if let Some((value, from)) = writer.value(id, name) {
            results.push((*name, cast(&value, from, *to)));
        }
    }
    problems.append(&mut writer.problems);
    Some(HookCode {
        helpers: writer.helpers,
        lines: writer.lines,
        results,
    })
}

/// What ends a hook, from what its outputs come to.
type HookTail<'a> = dyn Fn(&[(&'static str, String)]) -> String + 'a;

/// What `graph` comes to as a shader.
///
/// # Errors
///
/// Returns everything that's wrong with it, by node.
pub fn graph_code(graph: &ShaderGraph) -> Result<String, Vec<String>> {
    let mut problems = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for node in &graph.nodes {
        if !ids.insert(node.id.as_str()) {
            problems.push(format!("there are two nodes called {}", node.id));
        }
    }
    for name in graph.outputs.keys() {
        if !GRAPH_OUTPUTS.iter().any(|(known, ..)| known == name) {
            problems.push(format!("there's no output called {name}"));
        }
    }
    if graph.outputs.is_empty() {
        problems.push("nothing goes to an output".to_owned());
    }
    let has_post = graph.outputs.contains_key("color");
    if has_post && graph.outputs.len() > 1 {
        problems.push("color is for a level's final pass, and goes alone".to_owned());
    }
    let mut code = String::from("// Made from a graph. Change the graph, not this.\n\n");
    let mut body = |hook: GraphHook, (head, start): (&str, &str), tail: &HookTail<'_>| {
        if let Some(made) = hook_lines(graph, hook, &mut problems) {
            code.push_str(&made.helpers);
            let (lines, end) = (made.lines, tail(&made.results));
            let _ = write!(code, "{head} {{\n{start}{lines}{end}}}\n\n");
        }
    };
    let last = |results: &[(&str, String)]| {
        let value = results.first().map_or("", |(_, value)| value.as_str());
        format!("    return {value};\n")
    };
    body(
        GraphHook::Vertex,
        ("fn shader_vertex(world: vec3<f32>) -> vec3<f32>", ""),
        &last,
    );
    body(
        GraphHook::Uv,
        (
            "fn shader_uv(uv: vec2<f32>, world: vec3<f32>) -> vec2<f32>",
            "",
        ),
        &last,
    );
    body(
        GraphHook::Surface,
        (
            "fn shader_surface(surface: Surface) -> Surface",
            "    let uv = surface.uv;\n    let world = surface.world;\n",
        ),
        &|results| {
            let mut out = String::from("    var out = surface;\n");
            for (name, value) in results {
                let _ = writeln!(out, "    out.{name} = {value};");
            }
            out.push_str("    return out;\n");
            out
        },
    );
    body(
        GraphHook::Post,
        (
            "fn shader_post(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32>",
            "",
        ),
        &last,
    );
    if problems.is_empty() {
        Ok(code)
    } else {
        Err(problems)
    }
}
