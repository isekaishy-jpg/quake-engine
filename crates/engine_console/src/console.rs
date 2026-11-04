//! The command buffer, command table, aliases and cvars.

use std::collections::BTreeMap;

use crate::cvar::{Cvar, CvarFlags};
use crate::tokenize::{split_commands, tokenize};

/// The most commands one [`Console::execute`] call will run, which stops an
/// alias that calls itself from freezing the engine.
const MAX_COMMANDS_PER_EXECUTE: usize = 100_000;

/// A command handler. State lives in the context `C`, not in the handler.
pub type Handler<C> = fn(&mut Console<C>, &mut C, &Args);

/// A command's tokens. Token 0 is the command name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Args {
    /// The tokens, including the command name.
    tokens: Vec<String>,
}

impl Args {
    /// The number of tokens, including the command name.
    #[must_use]
    pub fn count(&self) -> usize {
        self.tokens.len()
    }

    /// The token at `index`, or an empty string, as `Cmd_Argv` gives.
    #[must_use]
    pub fn get(&self, index: usize) -> &str {
        self.tokens.get(index).map_or("", String::as_str)
    }

    /// Everything after the command name, joined by spaces.
    #[must_use]
    pub fn rest(&self) -> String {
        self.tokens.get(1..).unwrap_or_default().join(" ")
    }
}

/// The console: a command buffer with its commands, aliases and cvars.
pub struct Console<C> {
    /// Text waiting to run.
    buffer: String,
    /// Registered commands by lower-case name.
    commands: BTreeMap<String, Handler<C>>,
    /// Aliases by lower-case name.
    aliases: BTreeMap<String, String>,
    /// Cvars by lower-case name.
    cvars: BTreeMap<String, Cvar>,
    /// Lines waiting for the application to show.
    output: Vec<String>,
    /// Set by `wait`: stop until the next [`Console::execute`].
    waiting: bool,
}

impl<C> Default for Console<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> Console<C> {
    /// A console with the built-in commands.
    #[must_use]
    pub fn new() -> Self {
        let mut console = Self {
            buffer: String::new(),
            commands: BTreeMap::new(),
            aliases: BTreeMap::new(),
            cvars: BTreeMap::new(),
            output: Vec::new(),
            waiting: false,
        };
        console.register("alias", alias_command);
        console.register("echo", echo_command);
        console.register("wait", wait_command);
        console.register("cmdlist", cmdlist_command);
        console.register("cvarlist", cvarlist_command);
        console
    }

    /// Registers a command.
    ///
    /// # Panics
    ///
    /// If `name` is already registered, which is a programming error.
    pub fn register(&mut self, name: &str, handler: Handler<C>) {
        let previous = self.commands.insert(name.to_lowercase(), handler);
        assert!(previous.is_none(), "command `{name}` registered twice");
    }

    /// Registers a cvar.
    ///
    /// # Panics
    ///
    /// If `name` is already registered, which is a programming error.
    pub fn register_cvar(&mut self, name: &str, default: &str, flags: CvarFlags) {
        let previous = self
            .cvars
            .insert(name.to_lowercase(), Cvar::new(default, flags));
        assert!(previous.is_none(), "cvar `{name}` registered twice");
    }

    /// The cvar called `name`, if registered.
    #[must_use]
    pub fn cvar(&self, name: &str) -> Option<&Cvar> {
        self.cvars.get(&name.to_lowercase())
    }

    /// Every command, alias and cvar name starting with `prefix`, sorted.
    #[must_use]
    pub fn completions(&self, prefix: &str) -> Vec<String> {
        let prefix = prefix.to_lowercase();
        let mut names: Vec<String> = self
            .commands
            .keys()
            .chain(self.aliases.keys())
            .chain(self.cvars.keys())
            .filter(|name| name.starts_with(&prefix))
            .cloned()
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Every cvar that's kept between runs, as its name and value.
    pub fn saved_cvars(&self) -> impl Iterator<Item = (&str, &str)> {
        self.cvars
            .iter()
            .filter(|(_, cvar)| cvar.flags().saved)
            .map(|(name, cvar)| (name.as_str(), cvar.value()))
    }

    /// Every alias, as its name and text.
    pub fn aliases(&self) -> impl Iterator<Item = (&str, &str)> {
        self.aliases
            .iter()
            .map(|(name, text)| (name.as_str(), text.as_str()))
    }

    /// Adds `text` after everything already buffered.
    pub fn add(&mut self, text: &str) {
        self.buffer.push_str(text);
        self.buffer.push('\n');
    }

    /// Adds `text` before everything already buffered, so it runs next.
    pub fn insert(&mut self, text: &str) {
        let mut combined = String::with_capacity(text.len() + 1 + self.buffer.len());
        combined.push_str(text);
        combined.push('\n');
        combined.push_str(&self.buffer);
        self.buffer = combined;
    }

    /// Adds a line of output.
    pub fn print(&mut self, line: impl Into<String>) {
        self.output.push(line.into());
    }

    /// Takes the output lines waiting to be shown.
    pub fn take_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.output)
    }

    /// Runs buffered commands until the buffer empties or `wait` is reached.
    pub fn execute(&mut self, context: &mut C) {
        self.waiting = false;
        let mut count = 0;
        while !self.buffer.is_empty() && !self.waiting {
            count += 1;
            if count > MAX_COMMANDS_PER_EXECUTE {
                self.buffer.clear();
                self.print("command buffer cleared: too many commands in one frame");
                return;
            }
            let buffer = std::mem::take(&mut self.buffer);
            let (command, rest) = split_commands(&buffer);
            self.buffer.push_str(rest);
            let command = command.to_owned();
            self.run_command(context, &command);
        }
    }

    /// Runs one command: a command, then an alias, then a cvar.
    fn run_command(&mut self, context: &mut C, command: &str) {
        let tokens = tokenize(command);
        let Some(first) = tokens.first() else {
            return;
        };
        let name = first.to_lowercase();
        let args = Args { tokens };
        if let Some(&handler) = self.commands.get(&name) {
            handler(self, context, &args);
        } else if let Some(text) = self.aliases.get(&name).cloned() {
            self.insert(&text);
        } else if self.cvars.contains_key(&name) {
            self.run_cvar(&name, &args);
        } else {
            self.print(format!("Unknown command \"{}\"", args.get(0)));
        }
    }

    /// Shows a cvar with no value given, or sets it.
    fn run_cvar(&mut self, name: &str, args: &Args) {
        if args.count() == 1 {
            if let Some(value) = self.cvars.get(name).map(|cvar| cvar.value().to_owned()) {
                self.print(format!("\"{name}\" is \"{value}\""));
            }
        } else if let Some(cvar) = self.cvars.get_mut(name) {
            cvar.set(args.get(1));
        }
    }
}

/// `alias name "commands"`: defines an alias, or lists them with no arguments.
fn alias_command<C>(console: &mut Console<C>, _context: &mut C, args: &Args) {
    if args.count() == 1 {
        let lines: Vec<String> = console
            .aliases
            .iter()
            .map(|(name, text)| format!("{name} : {text}"))
            .collect();
        console.output.extend(lines);
        return;
    }
    let text = args.tokens.get(2..).unwrap_or_default().join(" ");
    let _previous = console.aliases.insert(args.get(1).to_lowercase(), text);
}

/// `echo text`: prints its arguments.
fn echo_command<C>(console: &mut Console<C>, _context: &mut C, args: &Args) {
    console.print(args.rest());
}

/// `wait`: stops until the next frame.
fn wait_command<C>(console: &mut Console<C>, _context: &mut C, _args: &Args) {
    console.waiting = true;
}

/// `cmdlist`: lists the commands.
fn cmdlist_command<C>(console: &mut Console<C>, _context: &mut C, _args: &Args) {
    let names: Vec<String> = console.commands.keys().cloned().collect();
    console.output.extend(names);
}

/// `cvarlist`: lists the cvars and their values.
fn cvarlist_command<C>(console: &mut Console<C>, _context: &mut C, _args: &Args) {
    let lines: Vec<String> = console
        .cvars
        .iter()
        .map(|(name, cvar)| format!("{name} \"{}\"", cvar.value()))
        .collect();
    console.output.extend(lines);
}
