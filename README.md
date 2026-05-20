# CS2 Keybind Manager

Never run out of keyboard real estate for your chat binds again. **CS2 Keybind Manager** is a lightweight desktop tool that lets you create and manage multiple "pages" of custom chat messages in Counter-Strike 2, all mapped to the exact same keys.

Need one page for serious tactical strat-calling, one page for casual team banter, and another for post-round compliments? This app lets you cycle through them — or jump directly to one — instantly mid-match.

---

## Key Features

- **Unlimited Chat Pages.** Create as many custom profiles or "pages" as you want.
- **One-Button Paging.** Toggle through your pages sequentially in-game by pressing a single hotkey (default `F1`).
- **Direct-Jump Keys.** Optionally bind a dedicated key to each page (e.g., `F2` → Strat Calls, `F3` → Trash Talk) so you can land on a specific page from anywhere without cycling. Pressing a direct key always lands on the same page, so you always know where you are.
- **Live Preview.** See exactly what will be written to your config files before you click Export. No surprises, no hidden behavior.
- **Zero Performance Impact.** The app writes native CS2 configuration files. It uses zero computer resources while you are playing — you can close it after exporting.
- **100% VAC-safe.** This tool does not modify game memory, inject code, or hook into the game process. It only writes to standard text config files, making it safe on Valve, FACEIT, and Premier.

---

## How It Works

Managing layered keybinds by hand means writing a tangle of `bind` and `exec` commands and remembering to chain them in the right order. This app handles all of that behind a clean visual interface.

1. **Set Your Game Folder.** The app auto-detects your CS2 cfg directory at the default Steam path on first launch. If you've installed Steam somewhere unusual, click **Browse…** to point it at the right folder.
2. **Choose a Cycle-Pages Key.** Pick the key you want to press to step to the next page (default `F1`).
3. **Build Your Pages.**
   - Click **Add Page** to create a new layer (e.g., "Strat Calls").
   - Optionally give the page a **direct key** in the sidebar — pressing it always lands on this page.
   - Assign a key (e.g., `1`) and type the message you want it to send (e.g., *"Let's rush B, don't stop!"*).
   - Go to your next page, assign the same key, and give it a different message (e.g., *"Good half everyone!"*).
4. **Export to CS2.** Click the export button. The app formats and saves everything into your game's cfg directory.

---

## In-Game Usage

Once you've exported your pages, using them is seamless.

- **On Game Launch.** Page 1 is loaded automatically when CS2 starts (via `autoexec.cfg`).
- **Cycling Pages.** Press your designated cycle-pages key (e.g., `F1`) to step to the next page. Press again to keep cycling; the last page wraps back to the first.
- **Direct Jumps.** If a page has a direct key set, pressing it from any page lands you on that page immediately. This is the simplest way to know which page you're on: if you pressed F2 and you set F2 as Strat Calls' direct key, you're on Strat Calls.
- **Updating on the Fly.** Want to add a new phrase or change a keybind while playing? Alt-Tab out, make your changes, click **Export**, return to the game, open your console (`~`), and type `exec autoexec`. Your new settings apply instantly without restarting. (You can also just cycle the cycle-pages key away from and back to a page to reload its bindings.)

---

## Frequently Asked Questions

**Do I need to keep this app open while playing CS2?**
No. Once you click "Export to CS2", you can close the application. The game reads the saved settings directly from disk.

**Can I use this for things other than chat commands, like buy binds or crosshair swaps?**
Yes. While it's designed with `say` commands in mind, any valid CS2 console command can be typed into the message field.

**Will this overwrite my existing crosshairs, viewmodels, or custom binds?**
The app's own block in `autoexec.cfg` lives between clear marker comments and won't touch anything outside it, and our own page cfg files are prefixed `bindmgr_page_*.cfg`. However, **regular gameplay binds you've set elsewhere (in `config.cfg` or via in-game settings) can be overwritten by this app's binds at runtime** — once a page is loaded, its bind lines take effect for the duration of that session. Use the live preview to see exactly what's being bound before exporting. The app also creates a one-time `autoexec.cfg.bak` backup the first time it edits your autoexec.

**My binds didn't auto-load — what now?**
On most setups `autoexec.cfg` runs automatically on game launch. In rare cases it doesn't (usually a file-extension or path quirk). If that happens to you, add `+exec autoexec` to your CS2 launch options in Steam (Library → Counter-Strike 2 → Properties → Launch Options) and try again.

**Which page am I on after a game restart?**
Page 1. CS2 has no way to remember the active page across launches, so every fresh launch starts on the first page in your cycle. The most reliable way to know which page you're on mid-match is to assign each page a direct key and use it instead of cycling.

**Why isn't there an on-screen banner showing the active page?**
We tried. CS2's `screenmessage_show` console command (the only client-side mechanism that mirrors `echo` output to the HUD) is cheat-protected by Valve and refuses to take effect inside an actual match. The console-filter workaround is also locked down on official servers. There is currently no reliable, non-cheat, client-side way to render custom on-screen text in CS2 matchmaking. Direct-jump keys are the practical answer.

**Is this safe for FACEIT / Premier / VAC?**
Yes. The app writes only to standard text `.cfg` files in your CS2 cfg directory. It never touches the game process. This is the same mechanism Valve provides for player customization.
