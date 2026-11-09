// A GUI program on Windows: no console window next to the dialogs
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod action;
mod batch;
mod cmdline;
mod config;
mod defaults;
mod effects;
mod engine;
mod fonts;
mod gpu;
mod i18n;
mod i18n_settings;
mod install;
mod media;
mod paths;
mod proc;
mod report;
mod scale;
mod settings;
mod syspal;
mod theme;
mod trash;
mod ui;
mod units;
mod update;

use action::Action;
use config::Config;
use engine::Request;
use media::Kind;
use scale::ScaleMode;
use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;

const HELP: &str = "\
cmmt - Media Tools: ffmpeg front end for the file manager context menu

Usage: cmmt <action> [options] [files...]

Actions:
  compress  convert-video  convert-audio  extract-audio  delete-audio
  frames  webp  convert-image  compress-image  make-icon  frames-to-video  stats
  settings                                               the Settings window (also: run cmmt without arguments)
  run <id> [files...]                                    an own menu item of the Settings window
  install [--kde|--gnome|--xfce|--pcmanfm|--windows] [--no-copy] [--path]   add the menu items
  uninstall                                              remove them
  config [--init|--path|--defaults]                      presets.toml: every ffmpeg command, editable

Options:
  --cli              no window: print the progress to the console
  --yes              do not show the options dialog (use the options below / the defaults)
  --batch            gather files of several launches (Windows registry commands)
  --format F         output format of convert-*, extract-audio, frames
  --level L          compress: low | medium | high
  --codec C          compress: codec key from presets.toml (h264_cpu, h265_nv, ...)
  --limit MB         compress: fit the result into this size
  --scale S          compress: orig | auto | 720p | 50% | x0.5 ...
  --twopass          compress with --limit: two-pass encoding (CPU H.264 / HEVC)
  --audio A          compress: copy | none | aac | opus | mp3 | ac3 | eac3 | flac | alac | pcm16 | pcm24
  --tracks 2,3       compress: keep only these audio tracks; delete-audio: remove only these
                     (counted from 1; default: all)
  --merge            compress, delete-audio: mix the audio tracks that are kept into one
  --delete-original  delete the original file after a successful result (to the trash or
                     permanently, as in Settings > General)
  --size N --fps N --quality N --compression N          webp
  compress-image: --format source|jpg|webp|avif|jxl|png  --quality 1-100  or  --limit 500KB|2MB
                  --scale S  --rotate 90|180|270  --flip h|v|hv  --crop x,y,w,h (in % of the picture)
                  colours, -100..100: --brightness --contrast --saturation --gamma --temperature
                  --vibrance N  --hue -180..180  --shadows r,g,b  --highlights r,g,b
                  --sharpen 0-100  --vignette 0-100
                  --blur x,y,w,h[,strength 1-100][,gauss|box|pixels]  (several: separated by ;)
                  --text T (\\n: a new line)  --text-at x,y (% of the picture, the middle of the text)
                  --text-size % (of the longer side)  --text-color RRGGBB  --text-outline %[,RRGGBB]
                  --text-box RRGGBB[,opacity %]  --font \"Family[:Style]\" or a font file
                  meme: --top-text T  --bottom-text T  --caption strip|overlay  --caption-size %
                  --caption-font F  --caption-color RRGGBB[,RRGGBB] (the text, then the strips or the outline)
                  (the order: colours, sharpen, blur, text, crop, vignette, captions)
  make-icon:      --sizes 16,24,32,48,64,128,256  --square pad|crop
  frames-to-video (folders or pictures): --fps N  --codec C  --level low|medium|high
";

struct Cli {
    action: String,
    files: Vec<PathBuf>,
    opts: HashMap<String, String>,
    flags: Vec<String>,
}

impl Cli {
    fn parse(args: Vec<String>) -> Cli {
        const WITH_VALUE: &[&str] = &[
            "format",
            "level",
            "codec",
            "limit",
            "scale",
            "audio",
            "tracks",
            "size",
            "fps",
            "quality",
            "compression",
            "rotate",
            "flip",
            "crop",
            "sizes",
            "square",
            "brightness",
            "contrast",
            "saturation",
            "gamma",
            "temperature",
            "hue",
            "vibrance",
            "shadows",
            "highlights",
            "sharpen",
            "vignette",
            "blur",
            "text",
            "text-at",
            "text-size",
            "text-color",
            "text-outline",
            "text-box",
            "font",
            "top-text",
            "bottom-text",
            "caption",
            "caption-size",
            "caption-font",
            "caption-color",
        ];
        let mut it = args.into_iter();
        let mut cli = Cli { action: it.next().unwrap_or_default(), files: vec![], opts: HashMap::new(), flags: vec![] };
        while let Some(a) = it.next() {
            let Some(name) = a.strip_prefix("--") else {
                cli.files.push(PathBuf::from(a));
                continue;
            };
            if let Some((k, v)) = name.split_once('=') {
                cli.opts.insert(k.to_string(), v.to_string());
            } else if WITH_VALUE.contains(&name) {
                match it.next() {
                    Some(v) => cli.opts.insert(name.to_string(), v),
                    None => None,
                };
            } else {
                cli.flags.push(name.to_string());
            }
        }
        cli
    }

    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }

    fn opt(&self, name: &str) -> Option<&str> {
        self.opts.get(name).map(String::as_str)
    }
}

/// Files of a file manager that passes the selection in the environment (Nautilus scripts and relatives)
fn files_from_env() -> Vec<PathBuf> {
    for var in ["NAUTILUS_SCRIPT_SELECTED_FILE_PATHS", "NEMO_SCRIPT_SELECTED_FILE_PATHS", "CAJA_SCRIPT_SELECTED_FILE_PATHS"] {
        if let Ok(v) = std::env::var(var) {
            let list: Vec<PathBuf> = v.lines().filter(|l| !l.is_empty()).map(PathBuf::from).collect();
            if !list.is_empty() {
                return list;
            }
        }
    }
    Vec::new()
}

/// The request for `--yes` / `--cli`, built from the options (defaults for what is missing)
fn request_from(action: Action, cli: &Cli, cfg: &Config, first_kind: Option<Kind>) -> Result<Request, String> {
    let num = |k: &str, d: u32| cli.opt(k).and_then(|v| v.parse().ok()).unwrap_or(d);
    let codec = cli.opt("codec").unwrap_or("h264_cpu").to_string();
    let scale = match cli.opt("scale") {
        Some(s) => ScaleMode::from_code(s).or_else(|| scale::parse_typed(s)).ok_or_else(|| format!("bad --scale: {s}"))?,
        None => ScaleMode::Orig,
    };
    let audio = cli.opt("audio").unwrap_or(cfg.default_audio()).to_string();
    if cfg.audio_codec(&audio).is_none() {
        return Err(format!("bad --audio: {audio}"));
    }
    // --tracks 2,3: numbers from 1, kept as numbers from 0
    let track_list = match cli.opt("tracks") {
        Some(list) => {
            let numbers: Option<Vec<usize>> = list.split(',').map(|n| n.trim().parse::<usize>().ok().filter(|n| *n > 0).map(|n| n - 1)).collect();
            Some(numbers.filter(|v| !v.is_empty()).ok_or_else(|| format!("bad --tracks: {list}"))?)
        }
        None => None,
    };
    let tracks = engine::Tracks { keep: track_list.clone(), merge: cli.flag("merge") };
    if let Some(what) = action.convert_kind(first_kind) {
        let format = cli.opt("format").or_else(|| cfg.default_format(what)).ok_or("no formats")?;
        return Ok(Request::Convert { what, format: format.to_string() });
    }
    let level = || match cli.opt("level").unwrap_or("medium") {
        "low" => Ok(0),
        "medium" => Ok(1),
        "high" => Ok(2),
        other => Err(format!("bad --level: {other}")),
    };
    Ok(match action {
        Action::CompressImage => image_request(cli, cfg, scale)?,
        Action::MakeIcon => {
            let sizes = match cli.opt("sizes") {
                Some(list) => list
                    .split(',')
                    .map(|n| n.trim().parse::<u32>().ok().filter(|v| (1..=256).contains(v)))
                    .collect::<Option<Vec<u32>>>()
                    .ok_or_else(|| format!("bad --sizes: {list}"))?,
                None => cfg.icon.sizes.clone(),
            };
            let crop = match cli.opt("square").unwrap_or("pad") {
                "pad" => false,
                "crop" => true,
                other => return Err(format!("bad --square: {other}")),
            };
            Request::Icon { sizes, crop }
        }
        Action::FramesVideo => {
            let fps = match cli.opt("fps") {
                Some(f) => units::parse_decimal(f).filter(|v| (1.0..=240.0).contains(v)).ok_or_else(|| format!("bad --fps: {f}"))?,
                None => cfg.frames_video.fps,
            };
            if cfg.compress_codec(&codec).is_none() {
                return Err(format!("bad --codec: {codec}"));
            }
            Request::FramesVideo { fps, codec, level: level()? }
        }
        Action::Compress => match cli.opt("limit") {
            Some(l) => {
                let mb = units::parse_decimal(l).filter(|v| *v > 0.0).ok_or_else(|| format!("bad --limit: {l}"))?;
                Request::CompressLimit { mb, codec, scale, twopass: cli.flag("twopass"), audio, tracks }
            }
            None => Request::CompressPreset { level: level()?, codec, scale, audio, tracks },
        },
        // --merge alone: no track is removed, all of them are mixed
        Action::DeleteAudio => match (track_list, cli.flag("merge")) {
            (None, true) => Request::DeleteAudio { remove: Some(vec![]), merge: true },
            (remove, merge) => Request::DeleteAudio { remove, merge },
        },
        Action::Webp => Request::Webp {
            size: num("size", cfg.webp.size),
            fps: num("fps", cfg.webp.fps),
            quality: num("quality", cfg.webp.quality),
            compression: num("compression", cfg.webp.compression),
        },
        _ => return Err(format!("{} has no options", action.cli_name())),
    })
}

/// compress-image: the format, quality or size, turn, mirror and crop of the options
fn image_request(cli: &Cli, cfg: &Config, scale: ScaleMode) -> Result<Request, String> {
    let format = cli.opt("format").unwrap_or(engine::SOURCE).to_string();
    if format != engine::SOURCE && cfg.image_format(&format).is_none() {
        return Err(format!("bad --format: {format}"));
    }
    let target = match (cli.opt("limit"), cli.opt("quality")) {
        (Some(l), _) => engine::ImageTarget::Bytes(units::parse_size(l).ok_or_else(|| format!("bad --limit: {l}"))?),
        (None, Some(q)) => engine::ImageTarget::Quality(q.parse().ok().filter(|v| (1..=100).contains(v)).ok_or_else(|| format!("bad --quality: {q}"))?),
        (None, None) => engine::ImageTarget::Quality(80),
    };
    let turns = match cli.opt("rotate").unwrap_or("0") {
        "0" => 0,
        "90" => 1,
        "180" | "-180" => 2,
        "270" | "-90" => 3,
        other => return Err(format!("bad --rotate: {other}")),
    };
    let flip = cli.opt("flip").unwrap_or("");
    if !flip.chars().all(|c| c == 'h' || c == 'v') {
        return Err(format!("bad --flip: {flip}"));
    }
    let crop = match cli.opt("crop") {
        Some(text) => {
            let v: Option<Vec<f64>> = text.split(',').map(|n| units::parse_decimal(n.trim().trim_end_matches('%')).map(|v| v / 100.0)).collect();
            match v.as_deref() {
                Some(&[x, y, w, h]) if x >= 0.0 && y >= 0.0 && w > 0.0 && h > 0.0 && x + w <= 1.0 + 1e-9 && y + h <= 1.0 + 1e-9 => Some([x, y, w, h]),
                _ => return Err(format!("bad --crop: {text} (x,y,width,height in % of the picture)")),
            }
        }
        None => None,
    };
    let mut edit = engine::ImageEdit { turns, flip_h: flip.contains('h'), flip_v: flip.contains('v'), crop, ..Default::default() };
    edit_options(cli, &mut edit)?;
    let scale = if scale == ScaleMode::Auto { ScaleMode::Orig } else { scale };
    Ok(Request::CompressImage { format, target, scale, edit: Box::new(edit) })
}

/// "50,90" or "50%,90%" -> numbers
fn numbers(text: &str) -> Option<Vec<f64>> {
    text.split(',').map(|n| units::parse_decimal(n.trim().trim_end_matches('%'))).collect()
}

/// "ff8800" or "#FF8800"
fn colour(text: &str) -> Option<[u8; 3]> {
    let h = text.trim().trim_start_matches('#');
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// compress-image: colours, sharpness, vignette, blurred areas, text and captions
fn edit_options(cli: &Cli, edit: &mut engine::ImageEdit) -> Result<(), String> {
    let int = |k: &str, lo: i32, hi: i32| -> Result<i32, String> {
        match cli.opt(k) {
            None => Ok(0),
            Some(v) => v.trim().parse::<i32>().ok().filter(|n| (lo..=hi).contains(n)).ok_or_else(|| format!("bad --{k}: {v} ({lo}..{hi})")),
        }
    };
    let three = |k: &str| -> Result<[i32; 3], String> {
        match cli.opt(k) {
            None => Ok([0; 3]),
            Some(v) => match numbers(v).as_deref() {
                Some(&[r, g, b]) if [r, g, b].iter().all(|n| (-100.0..=100.0).contains(n)) => Ok([r as i32, g as i32, b as i32]),
                _ => Err(format!("bad --{k}: {v} (r,g,b, each -100..100)")),
            },
        }
    };
    edit.colours = engine::Colours {
        brightness: int("brightness", -100, 100)?,
        contrast: int("contrast", -100, 100)?,
        saturation: int("saturation", -100, 100)?,
        gamma: int("gamma", -100, 100)?,
        temperature: int("temperature", -100, 100)?,
        hue: int("hue", -180, 180)?,
        vibrance: int("vibrance", -100, 100)?,
        shadows: three("shadows")?,
        highlights: three("highlights")?,
    };
    edit.sharpen = int("sharpen", 0, 100)? as u32;
    edit.vignette = int("vignette", 0, 100)? as u32;
    if let Some(list) = cli.opt("blur") {
        for one in list.split(';').filter(|s| !s.trim().is_empty()) {
            let bad = || format!("bad --blur: {one} (x,y,w,h in % of the picture, then the strength and gauss, box or pixels)");
            let parts: Vec<&str> = one.split(',').map(str::trim).collect();
            let rect = numbers(&parts[..parts.len().min(4)].join(",")).ok_or_else(bad)?;
            let [x, y, w, h] = rect[..] else { return Err(bad()) };
            if !(x >= 0.0 && y >= 0.0 && w > 0.0 && h > 0.0 && x + w <= 100.0 + 1e-9 && y + h <= 100.0 + 1e-9) {
                return Err(bad());
            }
            let mut area = engine::BlurArea { rect: [x / 100.0, y / 100.0, w / 100.0, h / 100.0], kind: engine::BlurKind::Gauss, strength: 30 };
            for p in &parts[4..] {
                if let Some(kind) = engine::BlurKind::ALL.into_iter().find(|k| k.key() == *p) {
                    area.kind = kind;
                } else {
                    area.strength = p.parse().ok().filter(|s| (1..=100).contains(s)).ok_or_else(bad)?;
                }
            }
            edit.blurs.push(area);
        }
    }
    let font = |k: &str| -> Result<std::path::PathBuf, String> {
        match cli.opt(k) {
            None => Ok(Default::default()),
            Some(name) => fonts::find(&fonts::system(), name).ok_or_else(|| format!("bad --{k}: no font \"{name}\"")),
        }
    };
    if let Some(text) = cli.opt("text") {
        let mut t = engine::TextMark { text: text.replace("\\n", "\n"), at: [0.5, 0.9], font: font("font")?, ..Default::default() };
        if let Some(v) = cli.opt("text-at") {
            match numbers(v).as_deref() {
                Some(&[x, y]) if (0.0..=100.0).contains(&x) && (0.0..=100.0).contains(&y) => t.at = [x / 100.0, y / 100.0],
                _ => return Err(format!("bad --text-at: {v} (x,y in % of the picture)")),
            }
        }
        if let Some(v) = cli.opt("text-size") {
            t.size = units::parse_decimal(v.trim().trim_end_matches('%')).filter(|s| *s > 0.0 && *s <= 100.0).ok_or_else(|| format!("bad --text-size: {v}"))?
                / 100.0;
        }
        if let Some(v) = cli.opt("text-color") {
            t.color = colour(v).ok_or_else(|| format!("bad --text-color: {v} (RRGGBB)"))?;
        }
        if let Some(v) = cli.opt("text-outline") {
            let (n, c) = v.split_once(',').map_or((v, None), |(n, c)| (n, Some(c)));
            t.outline = n.trim().trim_end_matches('%').parse().ok().filter(|n| *n <= 50).ok_or_else(|| format!("bad --text-outline: {v}"))?;
            if let Some(c) = c {
                t.outline_color = colour(c).ok_or_else(|| format!("bad --text-outline: {v}"))?;
            }
        }
        if let Some(v) = cli.opt("text-box") {
            let (c, o) = v.split_once(',').map_or((v, None), |(c, o)| (c, Some(o)));
            t.boxed = true;
            t.box_color = colour(c).ok_or_else(|| format!("bad --text-box: {v}"))?;
            if let Some(o) = o {
                t.box_opacity = o.trim().trim_end_matches('%').parse().ok().filter(|n| *n <= 100).ok_or_else(|| format!("bad --text-box: {v}"))?;
            }
        }
        edit.texts.push(t);
    }
    if cli.opt("top-text").is_some() || cli.opt("bottom-text").is_some() {
        let strip = match cli.opt("caption").unwrap_or("strip") {
            "strip" => true,
            "overlay" => false,
            other => return Err(format!("bad --caption: {other} (strip or overlay)")),
        };
        let mut c = engine::Caption {
            top: cli.opt("top-text").unwrap_or("").replace("\\n", "\n"),
            bottom: cli.opt("bottom-text").unwrap_or("").replace("\\n", "\n"),
            strip,
            font: font("caption-font")?,
            ..Default::default()
        };
        if let Some(v) = cli.opt("caption-color") {
            let bad = || format!("bad --caption-color: {v} (RRGGBB[,RRGGBB])");
            let (ink, back) = v.split_once(',').map_or((v, None), |(a, b)| (a, Some(b)));
            let colors = c.colors_mut();
            colors[0] = colour(ink).ok_or_else(bad)?;
            if let Some(back) = back {
                colors[1] = colour(back).ok_or_else(bad)?;
            }
        }
        if let Some(v) = cli.opt("caption-size") {
            c.size = v.trim().trim_end_matches('%').parse().ok().filter(|n| (1..=50).contains(n)).ok_or_else(|| format!("bad --caption-size: {v}"))?;
        }
        edit.caption = Some(c);
    }
    Ok(())
}

/// Output of a command that is not a menu action. A program without a console (Windows)
/// shows it in a window instead of printing it.
fn emit(title: &str, lines: Vec<String>, code: i32) -> i32 {
    if cfg!(windows) {
        let start = ui::Start::Message { title: title.to_string(), lines, ok: code == 0 };
        let _ = ui::run(ui::Launch::plain(start, Arc::new(Config::load())));
    } else {
        for (i, l) in lines.iter().enumerate() {
            if code == 0 {
                println!("{l}");
            } else if i == 0 {
                eprintln!("cmmt: {l}");
            } else {
                eprintln!("{l}");
            }
        }
    }
    code
}

fn show(launch: ui::Launch) -> i32 {
    match ui::run(launch) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("cmmt: {e}");
            1
        }
    }
}

fn settings_window() -> i32 {
    show(ui::Launch::plain(ui::Start::Settings, Arc::new(Config::load())))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    paths::move_old_config();
    // Started without arguments from a file manager or the application menu (no terminal): the Settings window
    if args.is_empty() && !std::io::stdout().is_terminal() {
        std::process::exit(settings_window());
    }
    let mut cli = Cli::parse(args);
    let code = match cli.action.as_str() {
        "" | "help" | "--help" | "-h" => emit("cmmt", HELP.lines().map(str::to_string).collect(), 0),
        "version" | "--version" | "-V" => emit("cmmt", vec![format!("cmmt {}", env!("CARGO_PKG_VERSION"))], 0),
        "settings" => settings_window(),
        "install" => {
            let (code, lines) = install::install(&install::Options::from_flags(&cli.flags));
            emit("Install", lines, code)
        }
        "uninstall" => {
            let (code, lines) = install::uninstall(&install::Options::from_flags(&cli.flags));
            emit("Uninstall", lines, code)
        }
        "config" => config_command(&cli),
        // cmmt run <id> [files...]: the first word is the id of an own item
        "run" if cli.files.is_empty() => emit("cmmt", vec!["run: the id of the item is missing".into()], 2),
        "run" => {
            let id = cli.files.remove(0).to_string_lossy().to_string();
            run_action(Action::Stats, Some(id), cli)
        }
        name => match Action::from_cli(name) {
            Some(action) => run_action(action, None, cli),
            None => emit("cmmt", [format!("unknown action \"{name}\""), String::new()].into_iter().chain(HELP.lines().map(str::to_string)).collect(), 2),
        },
    };
    std::process::exit(code);
}

fn config_command(cli: &Cli) -> i32 {
    let path = paths::presets_file();
    if cli.flag("defaults") {
        return emit("presets.toml", config::DEFAULT_PRESETS.lines().map(str::to_string).collect(), 0);
    }
    if cli.flag("init") {
        if path.exists() {
            return emit("presets.toml", vec![format!("{} already exists", path.display())], 0);
        }
        return match config::init_presets() {
            Ok(p) => {
                emit("presets.toml", vec![format!("created {}", p.display()), "it holds every ffmpeg command of the program: edit what you need".into()], 0)
            }
            Err(e) => emit("presets.toml", vec![e], 1),
        };
    }
    emit("presets.toml", vec![path.display().to_string()], 0)
}

fn run_action(action: Action, custom: Option<String>, cli: Cli) -> i32 {
    let cfg = Arc::new(Config::load());
    let t = i18n::t();
    if let Some(id) = &custom {
        if cfg.settings.custom(id).is_none() {
            return emit("cmmt", vec![format!("no own item \"{id}\" in settings.toml")], 2);
        }
    }

    let mut files = cli.files.clone();
    if files.is_empty() {
        files = files_from_env();
    }
    let files: Vec<PathBuf> = files.into_iter().map(|p| std::path::absolute(&p).unwrap_or(p)).collect();
    let files = if cli.flag("batch") {
        match batch::gather(action.cli_name(), &files) {
            Some(all) => all,
            None => return 0, // another launch handles the whole selection
        }
    } else {
        files
    };
    if files.is_empty() {
        eprintln!("{}", t.no_files);
        return 1;
    }

    let tools = match (paths::find_tool("ffmpeg", &cfg.settings.ffmpeg), paths::find_tool("ffprobe", &cfg.settings.ffprobe)) {
        (Some(ffmpeg), Some(ffprobe)) => Some(ui::Tools { ffmpeg, ffprobe }),
        _ => None,
    };
    let first_kind = files.iter().find_map(|f| Kind::of(f));
    let headless = cli.flag("cli");
    let request = if !headless && !cli.flag("yes") {
        None
    } else if let Some(id) = &custom {
        Some(Ok(Request::Custom { id: id.clone() }))
    } else if action == Action::Stats {
        None
    } else {
        Some(request_from(action, &cli, &cfg, first_kind))
    };
    if headless {
        return run_console(action, request, &cfg, tools, &files, cli.flag("delete-original"));
    }
    let request = match request.transpose() {
        Ok(r) => r,
        Err(e) => return emit("cmmt", vec![e], 2),
    };
    let delete_original = cli.flag("delete-original");
    show(ui::Launch { start: ui::Start::Normal, action, custom_id: custom, files, cfg, tools, request, delete_original, first_kind })
}

/// `--cli`: the job without a window. `request` is None only for Stats.
fn run_console(
    action: Action,
    request: Option<Result<Request, String>>,
    cfg: &Arc<Config>,
    tools: Option<ui::Tools>,
    files: &[PathBuf],
    delete_original: bool,
) -> i32 {
    let t = i18n::t();
    let Some(tools) = tools else {
        eprintln!("{}", t.ffmpeg_missing);
        return 1;
    };
    let req = match request {
        None if action == Action::Stats => {
            for f in files {
                println!("== {}\n{}\n", f.display(), engine::stats_text(&tools.ffprobe, f));
            }
            return 0;
        }
        None => return 2,
        Some(Err(e)) => {
            eprintln!("cmmt: {e}");
            return 2;
        }
        Some(Ok(r)) => r,
    };
    for w in &cfg.warnings {
        eprintln!("{} {w}", t.config_warning);
    }
    let prog = engine::Progress::new(Box::new(|| {}), true);
    let job = engine::Job { cfg: cfg.clone(), ffmpeg: tools.ffmpeg, ffprobe: tools.ffprobe, prog: prog.clone(), delete_original };
    engine::run(&job, &req, files);
    println!();
    let st = prog.snapshot();
    if st.errors > 0 || st.cancelled {
        1
    } else {
        0
    }
}
