# Terminal input validation — issue #2

2026-10-14, branch `issue/2-terminal`. This finding applies to the commit that
adds this file; dependency revisions remain those in `milestone-0.md` (Ghostty
`291fd050b9e9c293c8adb6a140742649b7e42d3a`, GPUI 0.2.2).
The user confirmed the repaired Enter path works. This is not IME acceptance.

## Implemented and executable checks

- Application-enabled mouse tracking now owns left/middle/right press/release,
  button/any-event motion, and vertical/horizontal wheel events. Ghostty encodes
  negotiated X10, UTF-8, SGR, URxvt and SGR-pixel formats. Disabled tracking keeps
  local selection and scrollback. Shift-drag and Shift-scroll explicitly override
  application tracking; Cmd+C still copies the local selection.
- A persistent encoder deduplicates motion within a cell. Updating its geometry
  or syncing terminal output resets that cache in the pinned API. Wheel deltas
  accumulate fractional lines rather than discarding small trackpad events;
  application wheel bursts are capped at 100 reports per axis/event.
- Composition preserves the native UTF-16 selected range and replacement range,
  paints local preedit without sending it to the PTY, and anchors candidate bounds
  to the rendered terminal cursor cell instead of the whole terminal viewport.
  The terminal cursor dims when focus/window activation is absent.
- `python3 scripts/check_vt.py` builds the bridge with `-Wall -Wextra -Werror`
  and AddressSanitizer/UndefinedBehaviorSanitizer. Assertions cover disabled
  tracking, SGR modifiers/releases/wheel, button and any-event motion/dedup,
  X10 press/suppressed release, UTF-8, URxvt and SGR-pixel encoding, plus the
  existing keyboard/paste/Unicode/selection/alternate-screen/retention checks.
- The pinned Ghostty mode 1016 encoder emits surface pixel coordinates directly
  (`9,18` becomes `CSI <0;9;18M`); the check records this upstream behavior.
- `cargo test --locked --features native --bin terminal-spike` compiles native
  input/render code and passes the existing physical-key routing regression test.
  These commands do not open a GUI. No shared `.spike` evidence is overwritten.

## Remaining native acceptance and concrete limits

Run native GUI checks sequentially after integration. No native GUI test or new
physical/IME/VoiceOver acceptance was performed for this change.

1. Disposable OpenCode/fixture: click, drag, release, wheel/trackpad and hover;
   verify application navigation and no simultaneous local scroll/selection.
   Test Shift-drag + Cmd+C and Shift-scroll while reporting is enabled, including
   changing modifiers during a drag and releasing outside the terminal.
   Native release-outside capture still needs verification; navigation/extra
   mouse buttons are not forwarded. Alternate-screen applications which do not
   request mouse tracking do not get synthesized wheel arrow keys.
2. Compose, move selection, replace, cancel and commit dead-key/CJK text; verify
   candidate placement after terminal resize and tab switches. Check UTF-16
   surrogate pairs/combining marks and absence of duplicate or premature input.
   The composition buffer only represents active preedit, not shell edit-line
   history; committed-text replacement outside it is unsupported. Preedit uses
   the existing fixed-cell renderer and does not paint internal selection/caret.
3. Check visible focused/dimmed cursor, terminal/tab click focus, and window
   deactivation/reactivation. DEC 1004 focus-in/out reports are still unsupported;
   no protocol acceptance is claimed. Candidate anchoring needs a visible cursor
   from the latest frame. Full cursor shapes/blink and function-key/key-release
   protocol coverage remain open.
4. Run Accessibility Inspector/VoiceOver against the exact integrated build.
   Custom painted cells still have no screen-reader text/navigation semantics.
   Native composition and screen-reader acceptance remain open.
5. Re-run milestone-0 Unicode, clipboard, selection, resize, alternate-screen,
   scrollback and retained-two-terminal scenarios, and measure visible/hidden
   heavy-output CPU/latency. The C parser timing is not a UI performance result.

This supersedes the earlier milestone-0 statement that mouse handlers always
select/scroll locally; it does not close the milestone-0 gate or issue #2.
