# Making the UI and menus usable

Research for items 1, 2 and 6 of the [work list](work-list.md), 2026-11-06. The human's word: the first attempt didn't use the UI pack well, and real effort and research has to go into making menus usable and good.

## What the pack is meant to look like

The pack's own previews are the design, and the first attempt ignored them.

- **The title screen** is the `conback` picture, a title with its letters spaced, and plain text rows. The chosen row is bright, with the kris pointing at it and a smear behind it. No boxes.
- **Settings** are a panel with tabs down the left, a heading, labels, and real controls: fields, toggles, checkboxes, sliders with a thumb and a number beside them.
- **The kit** has 84 parts, of which the first attempt used about four: frames for dialogs, tabs, fields, sliders, toggles, scroll bars, separators, icons, and a spinner. Its palette is in `theme.json`.

## What's known about good menus

Game UX has less settled research than other fields, as [one Drexel thesis](https://researchdiscovery.drexel.edu/esploro/outputs/graduate/The-UX-of-settings-menus-in/991020340714604721) says. Its A/B test found that applying the 7 Principles of Universal Design to a settings menu went with a better experience. Only its abstract could be read. So these lean on published accessibility guidelines, which have numbers, and treat blogs as a list of faults to check for.

### Numbers, from the Xbox Accessibility Guidelines

From [XAG 101](https://learn.microsoft.com/en-us/gaming/accessibility/xbox-accessibility-guidelines/101) and [XAG 102](https://learn.microsoft.com/en-us/gaming/accessibility/xbox-accessibility-guidelines/102):

| Rule | Number |
|---|---|
| Least text size on PC, by body height | 18 pixels at 1080p, 36 at 4K |
| Text scaling | Up to 200%, with nothing lost, and scrolling one way at most |
| Icons | Scale with text |
| Contrast of ordinary text and what matters | 4.5:1 |
| Contrast of large text and elements | 3:1 |
| Contrast of inactive text | 3:1 |
| High contrast mode | 7:1 for all UI |
| Text over a picture | Measured against its worst patch |
| Blocks of text | 80 characters at most, line spacing 1.5, left aligned |
| Full capitals | Fine for one or two words, not for sentences |

Other points: an outline or a solid plate behind text keeps it readable over a changing scene, stylised fonts need a plain option, and no text belongs in images but logos.

### From the Game Accessibility Guidelines

From [the basic list](https://gameaccessibilityguidelines.com/basic/): remappable controls, sensitivity, all UI reachable by the input used to play, starting the game without going through layers of menus, large well spaced controls, high contrast, nothing told by colour alone, simple clear language, separate volumes, all settings remembered, a wide choice of difficulty, and accessibility features described in the game.

### Rebinding keys

Practitioner guidance, not research: [detect a clash when binding, and swap or clear it](https://bugnet.io/blog/how-to-fix-key-rebinding-conflicts), keep essential actions bound, let capture be cancelled, keep menu keys safe, and [keep defaults so that they can be restored](https://uhiyama-lab.com/en/notes/unity/unity-input-rebinding-guide/). [A real fault](https://empyriononline.com/posts/385537) is a UI that shows one binding while another still works.

### Settings and pause menus

[Apple's guidance](https://developer.apple.com/design/human-interface-guidelines/settings.md) says to start from defaults that suit the most people. [A list of pause menu faults](https://www.wayline.io/blog/20-reasons-your-pause-menu-is-sabotaging-your-game) is opinion, and useful to check against: a hidden or unreliable way back to the game, unorganised settings, no remapping, tiny text, audio sliders that don't respond as expected, button prompts that don't follow remapping, no FOV setting, no quit confirmation, a slow UI.

A display change should [apply as a preview that reverts by itself](https://www.seeles.ai/resources/blogs/unreal-game-settings-menu-enhanced-input-key-rebinding-guide) unless it's kept.

### The HUD

[Peacocke and others, 2018](https://www.yorku.ca/mack/ec2018.pdf), as reported by search results, found players did better with a HUD for health. That paper couldn't be read here. Guidance says fast games should default to plain readouts, each element takes attention, and critical text wants a margin from the screen's edge.

## What we have, against that

| Finding | Where we are | What to do |
|---|---|---|
| Start with no layers of menus | Two screens to start anything | Continue first, and New Game goes straight in |
| Colours | `MUTED` is 4.71:1 and `INK` 12.3:1 on the kit's surface. `ACCENT` is 3.4:1, fine for large text only. The kit's disabled grey is 2.4:1, under 3:1 | Keep those, use `ACCENT` for large things, and lighten disabled text |
| High contrast mode | None | A setting, with 7:1 throughout |
| Text scale to 200% | Usual text at 720p is 24 pixels, twice the 12 the guideline works out at there. `ui_scale` had no limit, giant views were unusable, and a limit still forced scrolling, because a bigger UI has less room | Scaling relative to the picture, so that every screen has the same room. Text stays over the guideline at every resolution, and there's no setting. The guideline's scaling to 200% isn't offered as such. The human's word: no setting for now, as the UI will be revisited |
| Text over a scene | Plain text on the HUD | A shadow, and a plate as a setting |
| Clashing keys | Two actions can share a key, silently | Swap, and say so |
| Capture | Nothing says what's going on | A prompt that says how to cancel and clear |
| Keys in help | Fixed words | Say the real keys |
| Sliders | No number, click only | A number beside each, dragging, and sound |
| Confirmation | None for reset, overwrite or leaving | Dialogs |
| Pause menu | The same list as the title | Resume first, and a way to leave the game |
| Difficulty | Quake's first level only | Chosen with a new game |
| Volumes | Sound and music | Add ambience |
| Coming back | Screens forget where you were | Back returns to the row you came from |
| Knowing what a row does | Nothing | A line of plain words for the chosen row |
| What can be pressed | Nothing | Hints along the foot |
| On-screen text | Engine lines print over the game | Only what's for the player shows. The rest goes to the console |

## Not covered

No gamepad exists in the engine, so menus are for keyboard and mouse, which is what the game's played with. A screen reader needs the platform's accessibility layer, and goes with the editor's. None of this has been tried by a person with a disability: that needs a person.
