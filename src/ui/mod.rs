//! The windows. One eframe window goes through the stages options form -> progress -> result,
//! or shows the Stats text, a message, or the Settings. Drawn by the program itself, so it
//! looks the same on every system and needs no GUI toolkit.
//!
//! A form window opens at its final size: before it is created the form is laid out once in an
//! invisible egui context (`forms::measure`), so the width follows the names of the files and the
//! height follows the content. Nothing is resized afterwards, the user may resize freely.

mod app;
mod dropdown;
mod forms;
mod image_forms;
mod paint;
mod running;
mod scale_field;
mod settings;
mod stats;
mod widgets;

use crate::action::Action;
use crate::config::Config;
use crate::effects;
use crate::engine::Request;
use crate::i18n::{self, T};
use crate::i18n_settings as si;
use crate::media::{self, Info, Kind};
use crate::theme;
use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;

/// What the window shows first
pub enum Start {
    /// The options form of the action (or straight to the job with `--yes`)
    Normal,
    /// The Settings window (`cmmt settings`, or cmmt started without arguments from a file manager)
    Settings,
    /// Text of a command that has no console to print to (Windows)
    Message { title: String, lines: Vec<String>, ok: bool },
}

/// ffmpeg and ffprobe
#[derive(Clone)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

pub struct Launch {
    pub start: Start,
    pub action: Action,
    /// An own menu item (`cmmt run <id>`)
    pub custom_id: Option<String>,
    pub files: Vec<PathBuf>,
    pub cfg: Arc<Config>,
    /// None = not found
    pub tools: Option<Tools>,
    /// Some = skip the options dialog (`--yes`)
    pub request: Option<Request>,
    /// `--delete-original` (with `--yes`)
    pub delete_original: bool,
    pub first_kind: Option<Kind>,
}

impl Launch {
    /// A window that is not about files (the Settings, a message)
    pub fn plain(start: Start, cfg: Arc<Config>) -> Launch {
        Launch { start, action: Action::Stats, custom_id: None, files: vec![], cfg, tools: None, request: None, delete_original: false, first_kind: None }
    }
}

/// What every stage of the window knows
struct Env {
    t: &'static T,
    cfg: Arc<Config>,
    action: Action,
    title: String,
    custom_id: Option<String>,
    files: Vec<PathBuf>,
    /// ffprobe about the first files
    infos: Vec<Info>,
    /// the number of audio tracks of every file (the windows that choose tracks)
    track_counts: Vec<usize>,
    first_kind: Option<Kind>,
    tools: Option<Tools>,
    /// the encoders of ffmpeg (Compress Image leaves out the formats it cannot make)
    encoders: Vec<String>,
    /// the videos of Frames to Video
    sets: Vec<image_forms::SetInfo>,
}

impl Env {
    fn is_stats(&self) -> bool {
        self.action == Action::Stats && self.custom_id.is_none()
    }
}

pub fn run(l: Launch) -> Result<(), String> {
    let Launch { start, action, custom_id, files, cfg, tools, request, delete_original, first_kind } = l;
    let t = i18n::t();
    let pictures = matches!(action, Action::CompressImage | Action::MakeIcon) && custom_id.is_none();
    let infos: Vec<Info> = match (&tools, &start) {
        (Some(tools), Start::Normal) => files
            .iter()
            .take(3)
            .filter_map(|f| {
                let mut info = media::probe(&tools.ffprobe, f)?;
                // a photo is shown turned by its EXIF orientation, which ffprobe leaves out of its size
                if let Some((w, h)) = pictures.then(|| media::picture_size(&tools.ffprobe, f)).flatten() {
                    (info.width, info.height) = (w, h);
                }
                Some(info)
            })
            .collect(),
        _ => vec![],
    };
    let chooses_tracks = matches!(action, Action::Compress | Action::DeleteAudio) && custom_id.is_none() && request.is_none();
    let track_counts = match (&tools, &start) {
        (Some(tools), Start::Normal) if chooses_tracks && files.len() > 1 => media::audio_counts(&tools.ffprobe, &files),
        _ => infos.iter().map(|i| i.audio.len()).collect(),
    };
    let title = match (&start, &custom_id) {
        (Start::Settings, _) => si::s().title.to_string(),
        (Start::Message { title, .. }, _) => title.clone(),
        (_, Some(id)) => cfg.settings.custom(id).map_or(id.clone(), |c| c.name.clone()),
        _ => action.title(t).to_string(),
    };
    let form = matches!(start, Start::Normal) && custom_id.is_none() && request.is_none();
    let encoders = match &tools {
        Some(tools) if form && action == Action::CompressImage => crate::engine::encoders(&tools.ffmpeg),
        _ => vec![],
    };
    let sets = match &tools {
        Some(tools) if form && action == Action::FramesVideo => crate::engine::frame_sets(&files)
            .into_iter()
            .map(|set| {
                let size = set.frames.first().and_then(|f| media::picture_size(&tools.ffprobe, f));
                image_forms::SetInfo { set, size }
            })
            .collect(),
        _ => vec![],
    };
    let env = Env { t, cfg, action, title, custom_id, files, infos, track_counts, first_kind, tools, encoders, sets };
    let scale = env.cfg.settings.ui_scale.clamp(0.5, 3.0) as f32;
    let transparent = effects::transparency_enabled(&env.cfg.settings);
    let (w, h) = initial_size(&env, &start, request.is_some());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([w * scale, h * scale])
            .with_min_inner_size([360.0 * scale, 160.0 * scale])
            .with_title("Media Tools")
            .with_app_id(crate::install::app_id())
            .with_icon(icon())
            .with_transparent(transparent),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "cmmt",
        options,
        Box::new(move |cc| {
            theme::install_fonts(&cc.egui_ctx);
            Ok(Box::new(app::App::new(env, start, request.map(|r| (r, delete_original)), &cc.egui_ctx, transparent)))
        }),
    )
    .map_err(|e| e.to_string())
}

/// The icon of the window (X11, Windows). Wayland takes it from the menu entry named like the
/// app id (see `install::app_id`).
fn icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../../assets/cmmt-256.png")).unwrap_or_default()
}

fn initial_size(env: &Env, start: &Start, direct: bool) -> (f32, f32) {
    match start {
        Start::Settings => (820.0, 620.0),
        Start::Message { .. } => (640.0, 300.0),
        Start::Normal if env.tools.is_none() => (520.0, 230.0),
        Start::Normal if env.is_stats() => (780.0, 560.0),
        // the progress window fits its content on its first frame
        Start::Normal if direct => (560.0, 200.0),
        Start::Normal => forms::measure(env),
    }
}
