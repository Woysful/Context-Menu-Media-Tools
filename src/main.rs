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
mod paths;
mod proc;
mod scale;
mod settings;
mod syspal;
mod theme;
mod ui;
mod units;

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
        const WITH_VALUE: &[&str] = &["format", "level", "codec", "limit", "scale", "size", "fps", "quality", "compression"];
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
    if let Some(what) = action.convert_kind(first_kind) {
        let format = cli.opt("format").or_else(|| cfg.default_format(what)).ok_or("no formats")?;
        return Ok(Request::Convert { what, format: format.to_string() });
    }
    Ok(match action {
        Action::Compress => match cli.opt("limit") {
            Some(l) => {
                let mb = units::parse_decimal(l).filter(|v| *v > 0.0).ok_or_else(|| format!("bad --limit: {l}"))?;
                Request::CompressLimit { mb, codec, scale, twopass: cli.flag("twopass") }
            }
            None => {
                let level = match cli.opt("level").unwrap_or("medium") {
                    "low" => 0,
                    "medium" => 1,
                    "high" => 2,
                    other => return Err(format!("bad --level: {other}")),
                };
                Request::CompressPreset { level, codec, scale }
            }
        },
        Action::DeleteSound => Request::DeleteSound,
        Action::Webp => Request::Webp {
            size: num("size", cfg.webp.size),
            fps: num("fps", cfg.webp.fps),
            quality: num("quality", cfg.webp.quality),
            compression: num("compression", cfg.webp.compression),
        },
        _ => return Err(format!("{} has no options", action.cli_name())),
    })
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
                eprintln!("mmt: {l}");
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
            eprintln!("mmt: {e}");
            1
        }
    }
}

fn settings_window() -> i32 {
    show(ui::Launch::plain(ui::Start::Settings, Arc::new(Config::load())))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Started without arguments from a file manager or the application menu (no terminal): the Settings window
    if args.is_empty() && !std::io::stdout().is_terminal() {
        std::process::exit(settings_window());
    }
    let mut cli = Cli::parse(args);
    let code = match cli.action.as_str() {
        "" | "help" | "--help" | "-h" => emit("mmt", HELP.lines().map(str::to_string).collect(), 0),
        "version" | "--version" | "-V" => emit("mmt", vec![format!("mmt {}", env!("CARGO_PKG_VERSION"))], 0),
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
        // mmt run <id> [files...]: the first word is the id of an own item
        "run" if cli.files.is_empty() => emit("mmt", vec!["run: the id of the item is missing".into()], 2),
        "run" => {
            let id = cli.files.remove(0).to_string_lossy().to_string();
            run_action(Action::Stats, Some(id), cli)
        }
        name => match Action::from_cli(name) {
            Some(action) => run_action(action, None, cli),
            None => emit("mmt", [format!("unknown action \"{name}\""), String::new()].into_iter().chain(HELP.lines().map(str::to_string)).collect(), 2),
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
            return emit("mmt", vec![format!("no own item \"{id}\" in settings.toml")], 2);
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
        return run_console(action, request, &cfg, tools, &files);
    }
    let request = match request.transpose() {
        Ok(r) => r,
        Err(e) => return emit("mmt", vec![e], 2),
    };
    show(ui::Launch { start: ui::Start::Normal, action, custom_id: custom, files, cfg, tools, request, first_kind })
}

/// `--cli`: the job without a window. `request` is None only for Stats.
fn run_console(action: Action, request: Option<Result<Request, String>>, cfg: &Arc<Config>, tools: Option<ui::Tools>, files: &[PathBuf]) -> i32 {
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
            eprintln!("mmt: {e}");
            return 2;
        }
        Some(Ok(r)) => r,
    };
    for w in &cfg.warnings {
        eprintln!("{} {w}", t.config_warning);
    }
    let prog = engine::Progress::new(Box::new(|| {}), true);
    let job = engine::Job { cfg: cfg.clone(), ffmpeg: tools.ffmpeg, ffprobe: tools.ffprobe, prog: prog.clone() };
    engine::run(&job, &req, files);
    println!();
    let st = prog.snapshot();
    if st.errors > 0 || st.cancelled {
        1
    } else {
        0
    }
}
