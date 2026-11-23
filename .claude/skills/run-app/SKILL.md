---
name: run-app
description: Launch the VLC GPUI app on macOS and drive it to see a change working - scratch settings, screen captures, and scripted clicks, hovers and keys. Use when asked to run, check or screenshot the app.
---

# Running and driving the app (macOS)

The app is a native GPUI window, so it is driven from the shell: `screencapture` to look,
and `drive` (`drive.swift` here, posting Quartz events) to click, hover and press keys. This
works when the process running the shell has Accessibility and Screen Recording access
(System Settings → Privacy & Security). If a capture comes back black or clicks do nothing,
that access is missing: tell the user, don't work around it.

**The clicks and keys go to the user's real screen.** Say before you start driving, keep
it short, and stop at once if the user interrupts.

## 1. Launch

```bash
.claude/skills/run-app/launch.sh "$SCRATCH/rvlc"
```

`$SCRATCH` is the session's scratchpad (never the user's config). The script builds with
`--features runtime_shaders`, compiles `drive` into `target/drive`, writes a `local.toml`
that skips onboarding, starts the app in the background with `RVLC_CONFIG_DIR` pointing at
that folder, and prints its pid. The log is `$SCRATCH/rvlc/app.log`. Give it ~4 s to open.

Anything that plays must be silent: mute the app (the speaker in the control bar, or
Settings) before opening media.

## 2. Look

```bash
screencapture -x "$SCRATCH/shot.png"                     # whole screen
screencapture -x -R 180,40,560,560 "$SCRATCH/part.png"   # a region, in screen points
```

Then Read the PNG. Always look at the capture before claiming anything.

## 3. Drive

```bash
target/drive click X Y      # left-click at screen point (X, Y)
target/drive hover X Y      # move only: opens submenus, shows tooltips
target/drive drag X1 Y1 X2 Y2  # press, drag in steps, release (Customize Player's drags)
target/drive key 53         # Escape; add `cmd` for Command (key 12 cmd = ⌘Q)
```

Coordinates are screen **points**. A Retina capture is in pixels: divide by the scale,
which is the capture's pixel width over the width of
`osascript -e 'tell application "Finder" to get bounds of window of desktop'`
(2880 px over 1440 pt is 2). When Read shows a downscaled image, it says the factor to
get back to pixels; apply that first, then divide by the scale.

Useful key codes: Return 36, Escape 53, Space 49, Tab 48, Backspace 51, arrows 123-126
(left, right, down, up). There is no text typing: dialogs that start with their text selected
take Return to accept it, or Backspace to empty it.

Submenus close if the pointer cuts across another item on the way: hover the item's row
first, then move sideways into the submenu.

The first click after launch can land while the window is still activating; capture after
each step and check it took. Wait ~1 s after a click before capturing.

## 4. State

The settings the app saved are in `$SCRATCH/rvlc/settings.toml` (debounced; give it a couple
of seconds). Read them to confirm what a click changed. Don't hand-edit them to set up a
case: a value the schema rejects is silently replaced by its default (the log warns), so
set things up through the app's own UI instead.

## 5. Stop

```bash
kill <pid>
```

Stop the app when done, and say in the report what was checked on screen and what wasn't.
