# Media Tools (`mmt`)

One small program that adds the Media Tools items (compress, convert, extract audio, …) to the
context menu of your file manager. It works on **Windows, KDE (Dolphin), GNOME (Files), XFCE (Thunar)
and Raspberry Pi OS (PCManFM)**, on x86 and ARM.

* **No dependencies except ffmpeg and ffprobe.** `mmt` is a single file (about 8 MB). The windows are drawn by
  the program itself, no Qt / GTK / Python has to be installed.
* **Every command is yours to edit.** The complete ffmpeg command of each function is a line of a text file,
  `presets.toml` (see below). Own menu items with any command can be added in the Settings window.
* **Nothing is overwritten.** If `clip_compressed.mp4` exists, the result is `clip_compressed (1).mp4`.

## Install

1. Get `ffmpeg` and `ffprobe` (package manager; on Windows put `ffmpeg.exe` and `ffprobe.exe` next to
   `mmt.exe` or into a folder in `PATH`).
2. Run `mmt`: double-click it and the **Settings** window opens (*General → Install*), or from a terminal:

   ```
   mmt install          # detects KDE / GNOME / XFCE / PCManFM (Windows: the Explorer menu)
   mmt install --kde    # or choose: --kde  --gnome  --xfce  --pcmanfm  --windows
   mmt uninstall
   ```

`install` does four things: it copies the program to `~/.local/bin/mmt` (Windows: `%LOCALAPPDATA%\mmt\mmt.exe`) so the
downloaded file can be deleted; it writes the menu items; it **adds that folder to `PATH`** (Linux: a marked block in
`~/.profile`, `~/.bashrc`, `~/.zshenv` and a fish `conf.d` file; Windows: the user `PATH`) so `mmt` works in a terminal after
the next login; and it adds **Media Tools** to the application menu / Start menu, which opens the Settings. `--no-copy` uses the
program where it is, `--no-path` leaves `PATH` alone. `uninstall` removes all of it.

| System | What is written | Notes |
|---|---|---|
| KDE | `~/.local/share/kio/servicemenus/mmt-*.desktop` | video items at the top level, submenus *Convert Audio* / *Convert Image* |
| GNOME | `~/.local/share/nautilus/scripts/Media Tools/` (small `sh` scripts) | right-click → **Scripts → Media Tools**. Files has no file type filter for scripts, so the program picks what fits (*Convert Audio* works for audio files and for videos). The scripts are sorted by name; *Settings → Menu* can number them (01, 02, …) to keep the order of the other menus |
| XFCE | actions `mmt-…` inside `~/.config/Thunar/uca.xml` | other actions of that file are kept; restart Thunar once (`thunar -q`) |
| Raspberry Pi OS, LXDE, LXQt | `~/.local/share/file-manager/actions/mmt-*.desktop` | PCManFM; restart it once (`pcmanfm -q`) |
| Windows | `%APPDATA%\mmt\mmt-menu.reg`, imported with `reg import` | classic context menu (on Windows 11: *Show more options*); `mmt-menu-uninstall.reg` removes the items |

## The items

| Item | What it does |
|---|---|
| **Compress Video** | One window, two tabs. **Presets**: compression level + codec (H.264 / HEVC / AV1 on CPU, Nvidia, AMD, Intel, Raspberry Pi) + optional resolution. **Size limit**: fit the result into N MB (Discord preset, two-pass for CPU H.264 / HEVC, resolution incl. *Auto*); GPU encoders are encoded in chunks with feedback so the size is exact. |
| Convert Video | MP4, AVI, MOV, MKV, M4V, WebM |
| Convert Audio | audio files: WAV, MP3, FLAC, AAC, OGG, WMA, M4A, Opus. Videos: re-encodes the audio tracks, the video is copied |
| Extract Audio | MP3, AAC, Opus, WAV, M4A, MKA |
| Delete sound | copies the video without audio |
| Make frame sequence | PNG / JPG / WebP frames into a new folder next to the video |
| Convert to WebP anim | size, fps, quality, compression |
| Convert Image | JPG, PNG, BMP, TIFF, GIF, WebP, TGA |
| Stats | `ffprobe` output of the files |

The window shows the progress (speed, time left, the chunk or pass), a log, and the size of the result against the limit. A job that
finished without problems closes itself after a few seconds.

## Settings (`mmt settings`, or *Media Tools* in the application menu)

* **Appearance**: theme (system / dark / light), **the colours of the system** (KDE, GNOME and Windows; the checkbox is
  disabled where the palette cannot be read, e.g. XFCE), accent colour, **window transparency and blur** (Windows 11 and KDE Plasma
  on Wayland; elsewhere the window is opaque), interface scale. Changes are previewed at once.
* **Menu**: switch off the standard items you do not need; add **own items**: a name, the kind of files, any ffmpeg command
  and the name of the result. *Save and update menu* rewrites the menu of your file manager.
* **General**: language, auto-close, paths of ffmpeg / ffprobe, buttons to open `presets.toml`, install / remove.

The settings are stored in `~/.config/mmt/settings.toml` (Windows: `%APPDATA%\mmt\`); you can edit that file by hand, too.

## The commands: `presets.toml`

Every function is one complete ffmpeg command line with placeholders, for example:

```toml
[compress.h264_cpu]
label = "H.264"
group = "cpu"
command = 'ffmpeg -n -i {input} {vf} -pix_fmt yuv420p -c:v libx264 -profile:v main -crf {q} {output}'
output = "{base}_compressed.mp4"
q = [22, 30, 35]            # {q}: low / medium / high level
```

```
mmt config --init      # writes the complete file with all commands to ~/.config/mmt/presets.toml (Windows: %APPDATA%\mmt\)
mmt config --defaults  # prints the built-in file
```

Edit the commands there; a change applies at the next start. The file may contain only what you changed: it is merged over the
built-in one (tables key by key, a value such as `command` is replaced as a whole), so a function that is missing from your file is
taken from the program. (A full copy never gets the new presets of a later version: delete the entries you did not touch if you want
them.) A mistake never breaks the program: the file is ignored and the window says what is wrong.

The command is split into words first (quotes work like in a shell), and the placeholders are replaced afterwards, so file names with
spaces or quotes are safe. Placeholders:

| | |
|---|---|
| `{input}` `{output}` | the file and the result (`output` names it: `{base}` = path without extension, `{name}`, `{dir}`, `{srcext}`, `{ext}` / `{fmt}` of the chosen format, `{mb}` = the size limit) |
| `{vf}` | `-vf <filters>` of the chosen resolution, or nothing; `{scale}` = the same filters with a trailing comma, to put into your own `-vf "…"` chain |
| `{q}` `{p}` | values of the compression level of the entry |
| `{render_node}` | the GPU device of AMD / Intel (VAAPI) |
| size limit only | `{rate}` = `-b:v -maxrate -bufsize` computed from the limit, `{pass}` two-pass options, `{audio}` audio options, `{seek}` `-ss` / `-t` of a chunk, `{overwrite}` `-n` or `-y` |
| WebP animation | `{fps}` `{quality}` `{compression}` `{size}` |

A `[entry.windows]` / `[entry.linux]` sub-table overrides the entry on that system only (VAAPI on Linux, AMF / QSV on Windows for AMD / Intel).
`group` and `backend` decide where an entry is listed; `rc_max`, `rc_buf`, `aim`, `bpp_min`, `twopass`, `chunked` tune the size limit mode
(comments in the file). A new format or preset is a new table:

```toml
[video_convert.gif]
text = "GIF"
ext = "gif"
command = 'ffmpeg -n -i {input} -an -vf fps=12,scale=480:-1 {output}'
output = "{base}.{ext}"

[limit.telegram]            # another size limit preset: inherits everything from HEVC [CPU]
preset = true
label = "Telegram — 50 MB"
inherit = "h265_cpu"
limit = 50
output = "{base}_telegram.mp4"
```

## Raspberry Pi

* **Which build:** `linux-aarch64` for a 64-bit Raspberry Pi OS (Pi 3 / 4 / 5), `linux-armv7` for the 32-bit one (`./build.sh linux-aarch64`).
* **Menu:** Raspberry Pi OS uses PCManFM: `mmt install` finds it (or `mmt install --pcmanfm`).
* **Encoding:** the CPU codecs work everywhere. A Pi 3 / 4 also has a hardware **H.264** encoder (listed as *GPU · Raspberry Pi*; it has no
  quality mode, so the levels are bitrates, see `compress.h264_pi`). A Pi 5 has no hardware video encoder.
* The window needs OpenGL (ES) 2: the default Mesa driver of Raspberry Pi OS provides it.

## Command line

```
mmt compress --cli --limit 20 --codec h265_cpu --twopass video.mp4     # no window, progress in the terminal
mmt compress --yes --level high --scale 720p video.mp4                 # window with progress only, no questions
mmt convert-audio --yes --format mp3 song.wav
mmt run gif video.mp4                                                  # an own item of the Settings
```

`--cli` `--yes` `--format` `--level` `--codec` `--limit` `--scale` `--twopass` … (`mmt help`). `MMT_LANG=ru|uk|en` forces the language, `MMT_THEME=light|dark` the theme.
On Windows the program has no console: `help`, `install` and similar show their text in a window, and `--cli` is for Linux terminals.

## Build

```
./build.sh                 # for this computer
./build.sh --all           # Linux x86_64 / aarch64 / armv7 and Windows x86_64 / aarch64, whatever can be built here
./build.sh linux-aarch64   # e.g. for a 64-bit Raspberry Pi
./build.sh --list          # what each target needs
```

The result is in `dist/`. Needs Rust (`rustup`). Cross builds need `cargo-zigbuild` + `zig` (one tool for every target; it also aims at an
old glibc, so the binaries run on old distributions) or a GNU cross compiler per target: `./build.sh --list` tells which. A native build needs a
glibc at least as new as the one of the target computer: build on the oldest system you want to support (or use zigbuild).
The fonts (JetBrains Mono, SIL Open Font License) are in `assets/fonts/`.

## What was tested

On Linux x86_64: all 50 unit tests, every action with a real ffmpeg, the size limit mode against the old shell scripts the program replaces
(the output files were byte-identical), all windows (rendered under Xvfb), edited commands, own items, install / uninstall in a scratch `$HOME`;
KDE, GNOME and XFCE were then tried by hand on real systems.
**Not tested:** Windows (build, Explorer menu, `PATH`, Start menu, acrylic blur), the blur on KDE Wayland (the code is written against the
protocol and cannot show anything without a real compositor), PCManFM / Raspberry Pi, the hardware encoder of the Pi, AMD / Intel encoding
(VAAPI, AMF, QSV), ARM builds. The generated menu files and scripts are covered by unit tests; the AMF / QSV / Raspberry Pi command lines in
`presets.toml` are sensible defaults: adjust them there if your hardware needs something else.
