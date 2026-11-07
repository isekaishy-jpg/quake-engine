# Proposal 0139: all of the UI as a script

Status: accepted and implemented, 2026-11-07, the [work list](../research/work-list.md), items 1, 2 and 6.

## Research

The human's word: finish moving the UI over, give menus proper mouse support, and then Quake's UI isn't needed. Then, on seeing the first try: it could do far better with the UI pack, and real effort and research has to go into making menus usable and good. And that a lot of text printing on the screen belongs on the console. Then, on the second: not satisfied with all the scrolling and overflowing, and the cause named: if text wants to be bigger, the resolution has to be, so the UI's scaling has to be relative. And the bars under health and armour aren't needed: gameplay UI beyond what we have can be made later.

[The research note](../research/ui-and-menus.md) has what was found: what the pack is meant to look like, numbers from the Xbox Accessibility Guidelines, the Game Accessibility Guidelines, a study of settings menus, and what we had against all of that.

## Design

**Everything over the game is `scripts/ui.luau`.** `hud.rs`, `menu.rs` and `quit_messages.rs` are gone. Quake runs on the same UI as everything else.

- **`GameScreen`** in `engine_app` only keeps what's going on: stats, words for the middle of the screen, the end of level, scores. The script gets all of it, with stats and what's held by name, from tables in the fixture.
- **What the script is told** grew: scores, save slots, the keys for each command it asks about in `key_commands`, messages, and typed text.
- **The mouse.** `menu_pointer` hears where the pointer is and if it clicked. Hovering chooses a row, a click does what it does, sliders follow a held button, the wheel moves the choice, and the right button goes back.

**Menus follow the pack's own previews.**

- The title screen is the backdrop, a spaced title, and plain rows, with the kris and the smear on the chosen one.
- Settings are the kit's panel, with tabs for Game, Video, Audio, Controls and Accessibility, and its toggles, sliders and fields. `ui.text` takes `tracking`, for spaced letters.
- Questions are the kit's dialog.

**Usability, from the research.**

- **Starting.** Continue loads the newest save, in one press. New Game picks a difficulty and a level on one screen.
- **The pause menu** is its own list, with Resume first, and a way to leave the game.
- **Nothing lost by accident.** Quitting, leaving, resetting and saving over a game all ask, with No chosen first.
- **Keys.** Capture says what it's for and how to cancel or clear. A key does one thing, so taking one from another action says so. Keys have plain names, and help says the player's real keys.
- **Knowing what's going on.** A line says what the chosen row does, hints along the foot say what can be pressed, sliders show their number, and going back lands on the row you came from.
- **Contrast.** `ui_contrast` drops the backdrop and makes all text bright. `hud_plate` puts a dark plate behind the status bar. Text over the game has a shadow.
- **Volumes:** ambience joins sound and music.

**Everything fits, and nothing scrolls.**

- **Scaling is relative.** `canvas_scale` is the picture's size over 640 by 360 units, in any fraction. Every screen has at least that room, and what's on it only grows with the resolution. A setting that made the UI bigger took room from the layout, which is what forced scroll bars and compact layouts, so `ui_scale` is gone and so are they. Body text is 10 units or more, which is 30 pixels at 1080p against the guideline's 18.
- **Rows fit themselves.** `lay_out_rows` sizes rows to the room there is. A list too long for one column takes two: Controls shows all 18 actions, and Load and Save all 12 slots. Left and Right cross between columns. On plain screens the two columns sit on a panel, so that they can be read over the backdrop's art.
- **Text is measured.** `ui.measure` gives a script real widths from the font. A line that can't wrap gets smaller to fit, down to 8 units, and nothing is cut with dots. Words wrap, and dialogs grow to what they hold. A row with more than two keys says how many more.
- **The backdrop keeps its shape,** covering the screen from the bottom right, where its art is.
- **A keyboard reaches every tab,** with Page Up and Page Down. Before this, only a click did.
- **Keys read as words,** such as Page Down, Delete and Comma.

**What's on the screen.** The console keeps what the engine says. `tell_player` passes on what the game says to the player, and the script shows that at the top left, fading. A player alone isn't told they've entered the game. The frame rate is in the kit's font.

**The status bar** is plain numbers, with no bars.

**A trap found on the way.** A game stopped before its keys were set saved `unbindall` and nothing else, so that player had no keys for good, and no way to reach a menu to fix it. An empty set of keys isn't saved now, and `check_bindings` gives a player with none the usual ones.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** with no `scripts/ui.luau` there's no UI, and the console is what's left, as a finished game has its `common.pkt`. No gamepad exists in the engine, so menus are for what the game is played with. With no size setting, a player who wants bigger text has no way to ask for it short of a bigger picture. A window smaller than 640 by 360 fits, with small text.
- **Acceptance criteria:** Quake and our own levels play with this UI. Every screen works by keyboard alone and by mouse alone, fits at every resolution and shape, and never scrolls. Nothing engine-side prints over the game.
- **Future extension:** none planned.

## Review

Tried in the real game, with real clicks: the title, pause, settings and dialog screens, a key taken by clicking, and a slider set by clicking, in a Quake level and in the proving ground. A clash said what it took. High contrast was looked at before this pass. The first tries at a large size fell apart, and the human called the giant views unusable, which is what led to relative scaling.

Every screen was then looked at in the real game at 1024 by 768, 1366 by 768, 1500 by 640, 1600 by 900, 800 by 1200 and 1920 by 1080, with the window at that size, and a key in the second column taken by a real click. A first walk proved nothing: `screenshot` always drew at 1280 by 720, and the contact sheet stretched it to shape. So `screenshot` now takes a size, and the walk checks that each picture is the size asked for.

Tests walk every screen in four shapes with keys and the pointer, and fail on a scroll bar, on text cut short, on anything leaving the screen, on a row of a long list not showing, or on a tab a keyboard can't reach. Scores for 16 players, long messages and long end-of-game text are held to the same. They measure text with a wide estimate, as the fonts aren't in the repository, so text crossing into its neighbour inside a panel is for eyes, not tests.

**Whether this is good to use needs a person,** and whether it works for people with disabilities needs them.
