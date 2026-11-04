# Proposal 0042: the drawn console

Status: accepted, 2026-11-04, increment 8.

## Research

- The human asked how to reach the console in the game, and it could only be reached from a terminal.
- `console.c` drops a console over half the screen, keeps a scrollback, shows the last few lines over the view for three seconds, and fills the screen with no game running.
- `keys.c` sends each key to the game, console or menu, but key ups always reach the game, so nothing stays held.
- The platform names keys by position, and has no text input. The original's `keyshift` table is the same idea.

## Design

- `engine_ui::ConsoleView` keeps the lines and what's typed, and draws on a canvas. Lines go to it as well as to standard output, so the terminal still works.
- It has history, tab completion from `Console::completions`, Page Up and Down, and the wheel.
- `Engine::key` routes keys, as `Key_Event` does. Repeats reach the menus and console but not bindings.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** typing follows a US layout, by key position, until the platform offers text.
- **Acceptance criteria:** tests cover typing, Shift, history, the lines over the view and sliding. A capture shows it over a game.
- **Future extension:** text input from the platform, and chat with online play.

## Review

See [review 0011](../reviews/0011-increment-8.md).
