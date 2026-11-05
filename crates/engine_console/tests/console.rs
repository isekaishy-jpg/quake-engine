//! Console behaviour, following Quake's `cmd.c` and `cvar.c`.

use engine_console::{Args, Console, CvarFlags, split_commands, tokenize};

/// What the test commands record.
#[derive(Default)]
struct Context {
    /// Each call's arguments, joined.
    calls: Vec<String>,
}

/// Records its arguments.
fn record(_console: &mut Console<Context>, context: &mut Context, args: &Args) {
    context.calls.push(args.rest());
}

/// A console with a `record` command, and an empty context.
fn console() -> (Console<Context>, Context) {
    let mut console = Console::new();
    console.register("record", record);
    (console, Context::default())
}

#[test]
fn tokens_split_on_whitespace_and_quotes_group_text() {
    assert_eq!(
        tokenize("  bind  x \"say hi there\" "),
        ["bind", "x", "say hi there"]
    );
    assert_eq!(tokenize("name \"\""), ["name", ""]);
}

/// A deliberate difference from `COM_Parse`, which also splits on `{}()':`.
/// See proposal 0012.
#[test]
fn punctuation_stays_in_its_token() {
    assert_eq!(tokenize("connect host:26000"), ["connect", "host:26000"]);
}

#[test]
fn comments_end_a_command() {
    assert_eq!(tokenize("echo a // ignored"), ["echo", "a"]);
    assert!(tokenize("// nothing").is_empty());
}

#[test]
fn an_unterminated_quote_takes_the_rest() {
    assert_eq!(tokenize("echo \"open"), ["echo", "open"]);
}

#[test]
fn commands_split_on_semicolons_outside_quotes_and_on_newlines() {
    assert_eq!(split_commands("a;b"), ("a", "b"));
    assert_eq!(split_commands("a\nb"), ("a", "b"));
    assert_eq!(split_commands("say \"a;b\";c"), ("say \"a;b\"", "c"));
    assert_eq!(split_commands("last"), ("last", ""));
}

#[test]
fn buffered_commands_run_in_order() {
    let (mut console, mut context) = console();
    console.add("record 1; record 2");
    console.add("record 3");
    console.execute(&mut context);
    assert_eq!(context.calls, ["1", "2", "3"]);
}

#[test]
fn inserted_text_runs_before_what_is_buffered() {
    let (mut console, mut context) = console();
    console.add("record last");
    console.insert("record first");
    console.execute(&mut context);
    assert_eq!(context.calls, ["first", "last"]);
}

#[test]
fn wait_stops_until_the_next_execute() {
    let (mut console, mut context) = console();
    console.add("record 1; wait; record 2");
    console.execute(&mut context);
    assert_eq!(context.calls, ["1"]);
    console.execute(&mut context);
    assert_eq!(context.calls, ["1", "2"]);
}

#[test]
fn aliases_expand_in_place() {
    let (mut console, mut context) = console();
    console.add("alias go \"record a; record b\"; go; record c");
    console.execute(&mut context);
    assert_eq!(context.calls, ["a", "b", "c"]);
}

#[test]
fn an_alias_loop_is_stopped_and_reported() {
    let (mut console, mut context) = console();
    console.add("alias loop loop; loop");
    console.execute(&mut context);
    let output = console.take_output();
    assert!(output.iter().any(|line| line.contains("too many commands")));
}

#[test]
fn names_ignore_case() {
    let (mut console, mut context) = console();
    console.register_cvar("Volume", "0.7", CvarFlags::default());
    console.add("RECORD x; VOLUME 0.5");
    console.execute(&mut context);
    assert_eq!(context.calls, ["x"]);
    assert_eq!(console.cvar("volume").expect("cvar").value(), "0.5");
}

#[test]
fn a_cvar_prints_with_no_value_and_sets_with_one() {
    let (mut console, mut context) = console();
    console.register_cvar("fov", "90", CvarFlags::default());
    console.add("fov; fov 110; fov");
    console.execute(&mut context);
    assert_eq!(
        console.take_output(),
        ["\"fov\" is \"90\"", "\"fov\" is \"110\""]
    );
    let cvar = console.cvar("fov").expect("cvar");
    assert_eq!(cvar.default_value(), "90");
    assert!((cvar.number() - 110.0).abs() < f32::EPSILON);
}

#[test]
fn a_non_numeric_cvar_reads_as_zero() {
    let (mut console, mut context) = console();
    console.register_cvar("name", "player", CvarFlags { saved: true });
    console.add("");
    console.execute(&mut context);
    let cvar = console.cvar("name").expect("cvar");
    assert!(cvar.number().abs() < f32::EPSILON);
    assert!(cvar.flags().saved);
}

#[test]
fn unknown_commands_are_reported_and_do_not_stop_the_buffer() {
    let (mut console, mut context) = console();
    console.add("nosuch; record after");
    console.execute(&mut context);
    assert_eq!(console.take_output(), ["Unknown command \"nosuch\""]);
    assert_eq!(context.calls, ["after"]);
}

#[test]
fn echo_and_lists_print() {
    let (mut console, mut context) = console();
    console.register_cvar("fov", "90", CvarFlags::default());
    console.add("echo hello world; cvarlist; alias a b; alias");
    console.execute(&mut context);
    assert_eq!(
        console.take_output(),
        ["hello world", "fov \"90\"", "a : b"]
    );
    console.add("cmdlist");
    console.execute(&mut context);
    assert!(console.take_output().contains(&"record".to_owned()));
}

#[test]
#[should_panic(expected = "registered twice")]
fn registering_a_command_twice_is_a_defect() {
    let (mut console, _context) = console();
    console.register("record", record);
}
