# What VLC collects and what leaves your computer

VLC has no telemetry. It never reports what you play, how you use it or that it crashed. This
page lists everything it records about a session and every way something can leave the
computer, all of which you turn on or start yourself (PRD S9.6).

## Kept on this computer

| What | Where | Default | Setting |
|---|---|---|---|
| Log | `logs/vlc.log` beside the settings, 5 files of 5 MB at most | On | General → Diagnostics → **Save a log** |
| File names in the log | in the log | Hidden: only the extension is written (`<redacted>.mkv`) | Privacy → **Hide file names in logs and crash reports** |
| Crash reports | `logs/crash-<time>-<pid>.txt`, the newest 10 | Written when VLC crashes | — |
| Playback statistics | in memory, for **Show Statistics** | Off | General → Diagnostics → **Collect playback statistics** (advanced) |
| History, playback positions, Open Recent | the library database | On | Privacy → History |

In logs and crash reports your home folder is always written as `~`.

Passwords are kept only in the OS keychain (the last.fm, OpenSubtitles, proxy and web remote
passwords and the OpenSubtitles sign-in). A stream link with a user name or password in it
(`https://me:secret@host/…`) plays as typed, but the history, the session and saved playlists
keep it without them (`https://host/…`), and Open Recent and Recent Streams leave it out;
databases written by earlier builds have them taken out once (PRD S9.7.3).

### Crash reports

A crash report holds the app's version, the OS, the GPU, the libVLC version, whether developer
mode was on, the thread, the place in the code that failed, its message and a backtrace. While
**Hide file names in logs and crash reports** is on (the default), every file name and folder is
taken out of it, keeping only the extension (`<path>.mkv`); only the source locations of the
app's own code stay. Turning the setting off allows file names in later reports.

The next launch says a report was saved and that nothing was sent. **Show Report** shows the
file. **Report on GitHub** opens a new issue on the project's GitHub page in your browser with the
report filled in; nothing is sent until you read it there and submit it yourself.

### Playback statistics

**Collect playback statistics** has libVLC count bitrates and dropped or lost pictures while
playing. The counts are only shown by **Show Statistics**, never saved or sent. It is off unless
you turn it on, and applies when VLC restarts.

## Sent over the network, when you turn it on

| What | Sends | Default |
|---|---|---|
| **Check for updates automatically** (General) | a request for the latest release's manifest on GitHub | Off |
| **Fetch titles and cover art from the internet** (Privacy) | nothing yet: libVLC 3 looks art up online only through Lua scripts, and none ship (`docs/legal.md`), so art comes from the file and its folder | Off |
| **Submit played tracks to last.fm** (Privacy) | title, artist and album of the music you play | Off |
| **Search OpenSubtitles** (Privacy) | the video's file hash and the words searched for | Off |
| Podcasts (Network) | requests for the feeds you subscribed to | none until you subscribe |
| Network streams | requests for the addresses you open | — |
| Web remote | serves a page to your local network only, behind a password | Off |
| **Stream** (main menu, File on macOS) | the media playing, to this computer only, or to your network behind an access code in the link | Only this computer |

**Copy Debug Info** (the `…` menu, Help on macOS) puts the versions, the OS, the GPU and your
preferences on the clipboard: no paths, and nothing is sent.

Settings written by a build before S9.6 are moved to these defaults once: file names become
hidden and playback statistics off. You can turn either back.
