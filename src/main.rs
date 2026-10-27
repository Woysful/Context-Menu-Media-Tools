// A GUI program on Windows: no console window next to the dialogs
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod action;
mod batch;
mod cmdline;
mod config;
mod effects;
mod engine;
mod gpu;
mod i18n;
mod i18n_settings;
mod install;
mod media;
mod scale;
mod syspal;
mod theme;
mod ui;

use action::Action;
use config::Config;
use engine::{ConvertKind, Request};
use scale::ScaleMode;
use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;

const HELP: &str = "\
mmt - Media Tools: ffmpeg front end for the file manager context menu

Usage: mmt <action> [options] [files...]

Actions:
  compress  convert-video  convert-audio  extract-audio  delete-sound
  frames  webp  convert-image  stats
  settings                                               the Settings window (also: run mmt without arguments)
  run <id> [files...]                                    an own menu item of the Settings window
  install [--kde|--gnome|--xfce|--pcmanfm|--windows] [--no-copy] [--no-path]   add the menu items
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
  --size N --fps N --quality N --compression N          webp
";

struct Cli {
    action: String,
    files: Vec<PathBuf>,
    opts: HashMap<String, String>,
    flags: Vec<String>,
}

impl Cli {
    fn parse(args: Vec<String>) -> Cli {
        let mut it = args.into_iter();
        let action = it.next().unwrap_or_default();
        let mut cli = Cli { action, files: vec![], opts: HashMap::new(), flags: vec![] };
        const WITH_VALUE: &[&str] =
            &["format", "level", "codec", "limit", "scale", "size", "fps", "quality", "compression"];
        let rest: Vec<String> = it.collect();
        let mut i = 0;
        while i < rest.len() {
            let a = &rest[i];
            if let Some(name) = a.strip_prefix("--") {
                if let Some((k, v)) = name.split_once('=') {
                    cli.opts.insert(k.to_string(), v.to_string());
                } else if WITH_VALUE.contains(&name) && i + 1 < rest.len() {
                    cli.opts.insert(name.to_string(), rest[i + 1].clone());
                    i += 1;
                } else {
                    cli.flags.push(name.to_string());
                }
            } else {
                cli.files.push(PathBuf::from(a));
            }
            i += 1;
        }
        cli
    }

    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
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

fn absolute(p: PathBuf) -> PathBuf {
    std::path::absolute(&p).unwrap_or(p)
}

fn default_format(list: &[(String, config::Format)]) -> Option<String> {
    list.iter().find(|(_, f)| f.default).or_else(|| list.first()).map(|(k, _)| k.clone())
}

fn parse_scale(text: &str) -> Option<ScaleMode> {
    ScaleMode::from_code(text).or_else(|| scale::parse_typed(text))
}

/// The request for `--yes` / `--cli`, built from the options (defaults for what is missing)
fn request_from(action: Action, cli: &Cli, cfg: &Config, first_kind: Option<media::Kind>) -> Result<Request, String> {
    let opt = |k: &str| cli.opts.get(k).cloned();
    let num = |k: &str, d: u32| opt(k).and_then(|v| v.parse().ok()).unwrap_or(d);
    let format_of = |list: &[(String, config::Format)]| opt("format").or_else(|| default_format(list)).ok_or("no formats".to_string());
    let scale = match opt("scale") {
        Some(s) => parse_scale(&s).ok_or_else(|| format!("bad --scale: {s}"))?,
        None => ScaleMode::Orig,
    };
    Ok(match action {
        Action::Compress => {
            if let Some(l) = opt("limit") {
                let mb: f64 = l.parse().map_err(|_| format!("bad --limit: {l}"))?;
                Request::CompressLimit { mb, codec: opt("codec").unwrap_or_else(|| "h264_cpu".into()), scale, twopass: cli.flag("twopass") }
            } else {
                let level = match opt("level").as_deref().unwrap_or("medium") {
                    "low" => 0,
                    "medium" => 1,
                    "high" => 2,
                    other => return Err(format!("bad --level: {other}")),
                };
                Request::CompressPreset { level, codec: opt("codec").unwrap_or_else(|| "h264_cpu".into()), scale }
            }
        }
        Action::ConvertVideo => Request::Convert { what: ConvertKind::Video, format: format_of(&cfg.video_convert)? },
        Action::ExtractAudio => Request::Convert { what: ConvertKind::Extract, format: format_of(&cfg.extract)? },
        Action::Frames => Request::Convert { what: ConvertKind::Frames, format: format_of(&cfg.frames)? },
        Action::ConvertImage => Request::Convert { what: ConvertKind::Image, format: format_of(&cfg.image)? },
        Action::ConvertAudio => {
            if first_kind == Some(media::Kind::Video) {
                Request::Convert { what: ConvertKind::VideoAudio, format: format_of(&cfg.video_audio)? }
            } else {
                Request::Convert { what: ConvertKind::AudioFile, format: format_of(&cfg.audio_file)? }
            }
        }
        Action::DeleteSound => Request::DeleteSound,
        Action::Webp => Request::Webp {
            size: num("size", cfg.webp.size),
            fps: num("fps", cfg.webp.fps),
            quality: num("quality", cfg.webp.quality),
            compression: num("compression", cfg.webp.compression),
        },
        Action::Stats => return Err("stats has no request".into()),
    })
}

/// Output of a command that is not a menu action. A program without a console (Windows)
/// shows it in a window instead of printing it.
fn emit(title: &str, lines: Vec<String>, code: i32) -> i32 {
    if cfg!(windows) {
        let launch = ui::Launch {
            start: ui::Start::Message { title: title.to_string(), lines, ok: code == 0 },
            action: Action::Stats,
            custom_id: None,
            files: vec![],
            cfg: Arc::new(Config::defaults()),
            ffmpeg: None,
            ffprobe: None,
            request: None,
            first_kind: None,
        };
        let _ = ui::run(launch);
    } else {
        for (i, l) in lines.iter().enumerate() {
            if code == 0 {
                println!("{l}");
            } else if i == 0 {
                eprintln!("mmt: {l}");
            } else {
                eprintln!("{l}");
            }
        }
    }
    code
}

fn settings_window() -> i32 {
    let cfg = Arc::new(Config::load());
    let launch = ui::Launch {
        start: ui::Start::Settings,
        action: Action::Stats,
        custom_id: None,
        files: vec![],
        cfg,
        ffmpeg: None,
        ffprobe: None,
        request: None,
        first_kind: None,
    };
    match ui::run(launch) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("mmt: {e}");
            1
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Started without arguments from a file manager or the application menu (no terminal): the Settings window
    if args.is_empty() && !std::io::stdout().is_terminal() {
        std::process::exit(settings_window());
    }
    let cli = Cli::parse(args);
    let code = match cli.action.as_str() {
        "" | "help" | "--help" | "-h" => emit("mmt", HELP.lines().map(str::to_string).collect(), 0),
        "version" | "--version" | "-V" => emit("mmt", vec![format!("mmt {}", env!("CARGO_PKG_VERSION"))], 0),
        "settings" => settings_window(),
        "install" | "uninstall" => {
            let (code, lines) = install::run(&cli.action, &cli.flags);
            emit(if cli.action == "install" { "Install" } else { "Uninstall" }, lines, code)
        }
        "config" => config_command(&cli),
        "run" => {
            // mmt run <id> [files...]: the first word is the id of an own item
            let mut cli = cli;
            if cli.files.is_empty() {
                emit("mmt", vec!["run: the id of the item is missing".into()], 2)
            } else {
                let id = cli.files.remove(0).to_string_lossy().to_string();
                run_action(Action::Stats, Some(id), cli)
            }
        }
        name => match Action::from_cli(name) {
            Some(action) => run_action(action, None, cli),
            None => emit("mmt", vec![format!("unknown action \"{name}\""), String::new()].into_iter().chain(HELP.lines().map(str::to_string)).collect(), 2),
        },
    };
    std::process::exit(code);
}

fn config_command(cli: &Cli) -> i32 {
    let path = config::user_file();
    if cli.flag("defaults") {
        return emit("presets.toml", config::DEFAULT_PRESETS.lines().map(str::to_string).collect(), 0);
    }
    if cli.flag("init") {
        if path.exists() {
            return emit("presets.toml", vec![format!("{} already exists", path.display())], 0);
        }
        return match config::init_presets() {
            Ok(p) => emit("presets.toml", vec![format!("created {}", p.display()), "it holds every ffmpeg command of the program: edit what you need".into()], 0),
            Err(e) => emit("presets.toml", vec![e], 1),
        };
    }
    emit("presets.toml", vec![path.display().to_string()], 0)
}

fn run_action(action: Action, custom: Option<String>, cli: Cli) -> i32 {
    let cfg = Arc::new(Config::load());
    let t = i18n::t();
    if let Some(id) = &custom {
        if !cfg.settings.custom.iter().any(|c| &c.id == id) {
            return emit("mmt", vec![format!("no own item \"{id}\" in settings.toml")], 2);
        }
    }

    let mut files = cli.files.clone();
    if files.is_empty() {
        files = files_from_env();
    }
    let files: Vec<PathBuf> = files.into_iter().map(absolute).collect();
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

    let ffmpeg = config::find_tool("ffmpeg", &cfg.settings.ffmpeg);
    let ffprobe = config::find_tool("ffprobe", &cfg.settings.ffprobe);
    let first_kind = files.iter().find_map(|f| media::kind_of(f));
    let custom_request = custom.as_ref().map(|id| Request::Custom { id: id.clone() });

    let headless = cli.flag("cli");
    let preset = if custom_request.is_some() {
        if headless || cli.flag("yes") { custom_request.clone() } else { None }
    } else if headless || cli.flag("yes") {
        request_from(action, &cli, &cfg, first_kind).ok()
    } else {
        None
    };
    if headless {
        return run_console(action, custom_request, &cli, &cfg, ffmpeg, ffprobe, files, first_kind);
    }
    let launch = ui::Launch { start: ui::Start::Normal, action, custom_id: custom, files, cfg, ffmpeg, ffprobe, request: preset, first_kind };
    match ui::run(launch) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("mmt: {e}");
            1
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_console(
    action: Action,
    custom: Option<Request>,
    cli: &Cli,
    cfg: &Arc<Config>,
    ffmpeg: Option<PathBuf>,
    ffprobe: Option<PathBuf>,
    files: Vec<PathBuf>,
    first_kind: Option<media::Kind>,
) -> i32 {
    let t = i18n::t();
    let (Some(ffmpeg), Some(ffprobe)) = (ffmpeg, ffprobe) else {
        eprintln!("{}", t.ffmpeg_missing);
        return 1;
    };
    if action == Action::Stats && custom.is_none() {
        for f in &files {
            println!("== {}\n{}\n", f.display(), engine::stats_text(&ffprobe, f));
        }
        return 0;
    }
    let req = match custom.map(Ok).unwrap_or_else(|| request_from(action, cli, cfg, first_kind)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("mmt: {e}");
            return 2;
        }
    };
    for w in &cfg.warnings {
        eprintln!("{} {w}", t.config_warning);
    }
    let prog = engine::Progress::new(Box::new(|| {}), true);
    let job = engine::Job { cfg: cfg.clone(), ffmpeg, ffprobe, prog: prog.clone() };
    engine::run(&job, &req, &files);
    println!();
    let st = prog.state.lock().unwrap();
    if st.errors > 0 || st.cancelled {
        1
    } else {
        0
    }
}
