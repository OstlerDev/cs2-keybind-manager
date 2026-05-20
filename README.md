# CS2 Keybind Manager

Never run out of keyboard real estate for your chat binds again. **CS2 Keybind Manager** is a lightweight desktop tool that lets you create and manage multiple "pages" of custom chat messages in Counter-Strike 2, all mapped to the exact same keys.

Need one page for serious tactical strat-calling, one page for casual team banter, and another for post-round compliments? This app lets you cycle through them instantly mid-match with a single keystroke.

---

## Key Features

- **Unlimited Chat Pages.** Create as many custom profiles or "pages" as you want.
- **One-Button Paging.** Toggle through your pages sequentially in-game by pressing a single hotkey (default `F1`).
- **Live Preview.** See exactly what will be written to your config files before you click Export. No surprises, no hidden behavior.
- **Zero Performance Impact.** The app writes native CS2 configuration files. It uses zero computer resources while you are playing — you can close it after exporting.
- **100% VAC-safe.** This tool does not modify game memory, inject code, or hook into the game process. It only writes to standard text config files, making it safe to use on Valve, FACEIT, and Premier.

---

## How It Works

Managing layered keybinds by hand means writing a tangle of `bind` and `exec` commands and remembering to chain them in the right order. This app handles all of that behind a clean visual interface.

1. **Set Your Game Folder.** Tell the app where your CS2 cfg directory is. The default Steam path is auto-detected.
2. **Choose a Toggle Key.** Pick the key you want to press to flip through your pages (default `F1`).
3. **Build Your Pages.**
   - Click **Add Page** to create a new layer (e.g., "Strat Calls").
   - Assign a key (e.g., `1`) and type the message you want it to send (e.g., *"Let's rush B, don't stop!"*).
   - Go to your next page, assign the same key, and give it a different message (e.g., *"Good half everyone!"*).
4. **Export to CS2.** Click the export button. The app formats and saves everything into your game's cfg directory.

---

## In-Game Usage

Once you've exported your pages, using them is seamless.

- **On Game Launch.** Page 1 is loaded automatically when CS2 starts (via `autoexec.cfg`). If it doesn't load, add `+exec autoexec` to your CS2 launch options in Steam — the app reminds you of this in the success toast after exporting.
- **Swapping Pages.** Press your designated toggle key (e.g., `F1`) to cycle to the next page. Press again to keep cycling; the last page wraps back to the first.
- **Updating on the Fly.** Want to add a new phrase or change a keybind while playing? Alt-Tab out, make your changes, click **Export**, return to the game, open your console (`~`), and type `exec autoexec`. Your new settings apply instantly without restarting.

---

## Frequently Asked Questions

**Do I need to keep this app open while playing CS2?**
No. Once you click "Export to CS2", you can close the application. The game reads the saved settings directly from disk.

**Can I use this for things other than chat commands, like buy binds or crosshair swaps?**
Yes. While it's designed with `say` commands in mind, any valid CS2 console command can be typed into the message field.

**Will this overwrite my existing crosshairs, viewmodels, or custom binds?**
No. The app cleanly places its instructions inside its own marker block at the bottom of `autoexec.cfg`, and writes its own page files prefixed `bindmgr_page_*.cfg`. Your other settings are not touched. The app also creates a one-time `autoexec.cfg.bak` backup the first time it edits your autoexec.

**Which page am I on after a game restart?**
Page 1. CS2 has no way to remember the active page across launches, so every fresh launch starts on the first page in your cycle.

**Is this safe for FACEIT / Premier / VAC?**
Yes. The app writes only to standard text `.cfg` files in your CS2 cfg directory. It never touches the game process. This is the same mechanism Valve provides for player customization.
