Implement media controls:
1) In the preload/injected JS, if `navigator.mediaSession` exists, define handlers for `play`, `pause`, `previoustrack`, `nexttrack` that click the player controls.
2) Provide robust fallbacks with selectors (examples):
   - Play/Pause: `document.querySelector('[aria-label="Play"]') || document.querySelector('[aria-label="Pause"]')`
   - Next: `document.querySelector('[aria-label="Next"]')`
   - Previous: `document.querySelector('[aria-label="Previous"]')`
3) In Rust, register global shortcuts (e.g. CmdOrCtrl+Alt+P for toggle, +N next, +B prev) using `tauri-plugin-global-shortcut` and send commands to the frontend via IPC.
4) Ensure hardware media keys also trigger the handlers if the plugin/OS emits them.
Deliver: Rust code (setup, commands/emit), injected JS, and frontend wiring.
