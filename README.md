# Media Tools (`cmmt`)

`cmmt` is short for **C**ontext **M**enu **M**edia **T**ools. Versions before were called `mmt`: installing this one
removes their menu items and moves `~/.config/mmt` to `~/.config/cmmt`.

One small program that adds the Media Tools items (compress, convert, extract audio, …) to the
context menu of your file manager. It works on **Windows, KDE (Dolphin), GNOME (Files), XFCE (Thunar)
and Raspberry Pi OS (PCManFM)**, on x86 and ARM.

* **No dependencies except ffmpeg and ffprobe.** On Linux `cmmt` is one AppImage (about 4 MB), on Windows one `.exe`.
  The windows are drawn by the program itself, no Qt / GTK / Python has to be installed.
* **Every command is yours to edit.** The complete ffmpeg command of each function is a line of a text file,
  `presets.toml` (see below). Own menu items with any command can be added in the Settings window.
* **Nothing is overwritten.** If `clip_compressed.mp4` exists, the result is `clip_compressed (1).mp4`.

## Install

### Linux: the AppImage

1. Install `ffmpeg` with the package manager (it brings `ffprobe`). The AppImage does not contain ffmpeg: it uses the one
   of the system, or `ffmpeg` / `ffprobe` placed next to the AppImage file.
2. Download the AppImage of your computer: `cmmt-linux-x86_64.AppImage` (PCs), `cmmt-linux-aarch64.AppImage`
   (64-bit ARM, Raspberry Pi with a 64-bit OS) or `cmmt-linux-armv7.AppImage` (32-bit Raspberry Pi OS).
3. Make it executable and start it: double-click it (most file managers ask to allow running it) and the **Settings**
   window opens; press **Install**. Or in a terminal:

   ```
   chmod +x cmmt-linux-x86_64.AppImage
   ./cmmt-linux-x86_64.AppImage install          # detects KDE / GNOME / XFCE / PCManFM
   ./cmmt-linux-x86_64.AppImage install --kde    # or choose: --kde  --gnome  --xfce  --pcmanfm
   ./cmmt-linux-x86_64.AppImage uninstall        # later: removes the menu items again
   ```

AppImages are mounted with FUSE (the `fuse3` or `fuse` package, present on nearly every desktop). Without it, set
`APPIMAGE_EXTRACT_AND_RUN=1`. The plain binary `cmmt-linux-x86_64` is also available; it works the same way.

### Windows

Put `ffmpeg.exe` and `ffprobe.exe` next to `cmmt.exe` or into a folder in `PATH`, double-click `cmmt.exe` and press
**Install** (or run `cmmt install`).

### What install does

`install` writes the menu items and adds **Media Tools** (it opens the Settings) to the application menu / Start menu.
The menu items call the program by its full path:

* **AppImage**: it stays where it is and the menu items point at it, so an AppImage kept up to date by Gear Lever or
  AppImageLauncher keeps working. If such a tool already made an application menu entry for this AppImage, `install`
  does not add a second one (and removes its own from an earlier install). Install from the AppImage at its final place:
  after moving it, press **Install** again.
* **Plain binary**: it is copied to `~/.local/bin/cmmt` (Windows: `%LOCALAPPDATA%\cmmt\cmmt.exe`), so the downloaded file can
  be deleted. `--no-copy` uses it where it is.

`PATH` is not touched unless you ask for it: `install --path` makes `cmmt` work in a terminal (an AppImage gets a link
`~/.local/bin/cmmt`; Linux: a marked block in `~/.profile`, `~/.bashrc`, `~/.zshenv` and a fish `conf.d` file; Windows: the
user `PATH`). `uninstall` removes all of it, the `PATH` blocks of earlier versions included.

| System | What is written | Notes |
|---|---|---|
| KDE | `~/.local/share/kio/servicemenus/cmmt-*.desktop` | video items at the top level, submenus *Convert Audio* / *Convert Image* |
| GNOME | `~/.local/share/nautilus/scripts/Media Tools/` (small `sh` scripts) | right-click → **Scripts → Media Tools**. Files has no file type filter for scripts, so the program picks what fits (*Convert Audio* works for audio files and for videos). The scripts are sorted by name; *Settings → Menu* can number them (01, 02, …) to keep the order of the other menus |
| XFCE | actions `cmmt-…` inside `~/.config/Thunar/uca.xml` | other actions of that file are kept; restart Thunar once (`thunar -q`) |
| Raspberry Pi OS, LXDE, LXQt | `~/.local/share/file-manager/actions/cmmt-*.desktop` | PCManFM; restart it once (`pcmanfm -q`) |
| Windows | `%APPDATA%\cmmt\cmmt-menu.reg`, imported with `reg import` | classic context menu (on Windows 11: *Show more options*); `cmmt-menu-uninstall.reg` removes the items |

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

## Settings (`cmmt settings`, or *Media Tools* in the application menu)

* **Appearance**: theme (system / dark / light), **the colours of the system** (KDE, GNOME and Windows; the checkbox is
  disabled where the palette cannot be read, e.g. XFCE), accent colour, **window transparency and blur** (Windows 11 and KDE Plasma
  on Wayland; elsewhere the window is opaque), interface scale. Changes are previewed at once.
* **Menu**: switch off the standard items you do not need; add **own items**: a name, the kind of files, any ffmpeg command
  and the name of the result. *Save and update menu* rewrites the menu of your file manager.
* **General**: auto-close, paths of ffmpeg / ffprobe (empty = next to the program, then `PATH`), buttons to open `presets.toml`.
  *Install* / *Remove* are at the top of the window.

The settings are stored in `~/.config/cmmt/settings.toml` (Windows: `%APPDATA%\cmmt\`); you can edit that file by hand, too.

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
cmmt config --init      # writes the complete file with all commands to ~/.config/cmmt/presets.toml (Windows: %APPDATA%\cmmt\)
cmmt config --defaults  # prints the built-in file
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

* **Which build:** `cmmt-linux-aarch64.AppImage` for a 64-bit Raspberry Pi OS (Pi 3 / 4 / 5), `cmmt-linux-armv7.AppImage` for the 32-bit one (`./build.sh linux-aarch64`).
* **Menu:** Raspberry Pi OS uses PCManFM: `cmmt install` finds it (or `cmmt install --pcmanfm`).
* **Encoding:** the CPU codecs work everywhere. A Pi 3 / 4 also has a hardware **H.264** encoder (listed as *GPU · Raspberry Pi*; it has no
  quality mode, so the levels are bitrates, see `compress.h264_pi`). A Pi 5 has no hardware video encoder.
* The window needs OpenGL (ES) 2: the default Mesa driver of Raspberry Pi OS provides it.

## Command line

`cmmt` in a terminal needs `install --path` (or call the AppImage / binary by its path).

```
cmmt compress --cli --limit 20 --codec h265_cpu --twopass video.mp4     # no window, progress in the terminal
cmmt compress --yes --level high --scale 720p video.mp4                 # window with progress only, no questions
cmmt convert-audio --yes --format mp3 song.wav
cmmt run gif video.mp4                                                  # an own item of the Settings
```

`--cli` `--yes` `--format` `--level` `--codec` `--limit` `--scale` `--twopass` … (`cmmt help`).
On Windows the program has no console: `help`, `install` and similar show their text in a window, and `--cli` is for Linux terminals.

## Build

```
./build.sh                 # for this computer
./build.sh --all           # Linux x86_64 / aarch64 / armv7 and Windows x86_64 / aarch64, whatever can be built here
./build.sh linux-aarch64   # e.g. for a 64-bit Raspberry Pi
./build.sh --list          # what each target needs
```

The result is in `dist/`: for Linux the AppImage (`cmmt-linux-<arch>.AppImage`) and the plain binary, for Windows the `.exe`.
The AppImage is packed with `appimagetool` (from `APPIMAGETOOL=…`, from `PATH`, or downloaded once into `target/tools/`);
`--no-appimage` skips it. Needs Rust (`rustup`). Cross builds need `cargo-zigbuild` + `zig` (one tool for every target; it also aims at an
old glibc, so the binaries run on old distributions) or a GNU cross compiler per target: `./build.sh --list` tells which. A native build needs a
glibc at least as new as the one of the target computer: build on the oldest system you want to support (or use zigbuild).
The fonts (JetBrains Mono, SIL Open Font License) are in `assets/fonts/`.

## What was tested

On Linux x86_64: all unit tests (63), the x86_64 AppImage (build, start, install from it in a scratch `$HOME`), every action with a real ffmpeg, the size limit mode against the old shell scripts the program replaces
(the output files were byte-identical), all windows (rendered under Xvfb), edited commands, own items, install / uninstall in a scratch `$HOME`;
KDE, GNOME and XFCE were then tried by hand on real systems.
**Not tested:** Windows (build, Explorer menu, `PATH`, Start menu, acrylic blur), the blur on KDE Wayland (the code is written against the
protocol and cannot show anything without a real compositor), PCManFM / Raspberry Pi, the hardware encoder of the Pi, AMD / Intel encoding
(VAAPI, AMF, QSV), ARM builds and ARM AppImages. The generated menu files and scripts are covered by unit tests; the AMF / QSV / Raspberry Pi command lines in
`presets.toml` are sensible defaults: adjust them there if your hardware needs something else.
