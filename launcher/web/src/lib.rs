//! Horizon's launcher in the browser: the fork's releases (Downloads), the pinned
//! Minecraft's version file, client jar and asset index straight from Mojang (Minecraft,
//! as the desktop launcher's *Download directly* does them, checked and resumable), and a
//! folder's check against the same pins. The desktop launcher's GPUI does not compile to
//! wasm; this edition shares its design tokens (`horizon-theme`), its pins
//! (research-lock.json) and its checks, on egui's glow backend (WebGL2, the oldest machines'
//! browser).
//!
//! ```text
//! cargo run -p horizon-web            # the pages, native: every browser power answers a note
//! cargo build --target wasm32-unknown-unknown --release   # the build the site serves
//! ```

mod glue;
mod lang;
mod manifest;
mod pins;
mod platform;

use eframe::egui;
use horizon_theme::glass;
use lang::{Key, Lang};
use sha1::{Digest, Sha1};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// `Download directly` stops between files when this is what stopped it.
const CANCELLED: &str = "cancelled";

/// Horizon's text colour, the glass look's white on its dark pages.
const TEXT: egui::Color32 = egui::Color32::from_rgb(0xf2, 0xef, 0xfc);

// ── The state the pages show, shared with the tasks that fill it ─────────────────────────

#[derive(PartialEq, Clone, Copy)]
enum Page {
    Home,
    Downloads,
    Minecraft,
}

enum Releases {
    NotAsked,
    Loading,
    Loaded(Vec<Release>),
    Failed(String),
}

#[derive(serde::Deserialize, Clone)]
struct GhRelease {
    tag_name: String,
    name: Option<String>,
    published_at: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(serde::Deserialize, Clone)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// One release, its Horizon files separated from the rest.
#[derive(Clone)]
struct Release {
    name: String,
    published: String,
    prerelease: bool,
    assets: Vec<GhAsset>,
}

#[derive(PartialEq, Clone)]
enum Folder {
    /// Not chosen yet.
    None,
    Ready,
    /// This browser has no File System Access API.
    Unsupported,
    Failed(String),
}

enum Download {
    Idle,
    Running {
        done: usize,
        total: usize,
        /// Bytes so far, of all of them; the bar's length.
        bytes: u64,
        total_bytes: u64,
        current: String,
        log: Vec<String>,
    },
    Done(Vec<String>),
    Failed(String),
}

#[derive(Clone, Copy)]
enum Outcome {
    Matches,
    Differs,
    Missing,
}

struct Verify {
    client: Outcome,
    version_file: Outcome,
    asset_index: Outcome,
}

/// What the pages show. The tasks hold it too (an `Rc` they and the app share), so a download
/// half way and the folder picker write into the same place the frames read.
struct Inner {
    releases: Releases,
    folder: Folder,
    download: Download,
    verify: Option<Verify>,
    /// Set by Cancel: the download under way stops between files.
    cancel: bool,
    /// A folder check is reading (fast; keeps the frames coming until it is done).
    verifying: bool,
}

impl Inner {
    fn busy(&self) -> bool {
        matches!(self.download, Download::Running { .. })
            || matches!(self.releases, Releases::Loading)
            || self.verifying
    }
}

// ── The app ─────────────────────────────────────────────────────────────────────────────

pub struct App {
    lang: Lang,
    page: Page,
    inner: Rc<RefCell<Inner>>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext) -> App {
        install_fonts_and_style(&cc.egui_ctx);
        App {
            lang: Lang::from_browser(),
            page: Page::Home,
            inner: Rc::new(RefCell::new(Inner {
                releases: Releases::NotAsked,
                folder: Folder::None,
                download: Download::Idle,
                verify: None,
                cancel: false,
                verifying: false,
            })),
        }
    }

    fn home(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.add_space(28.);
        wordmark(ui, "HORIZON", 34.);
        ui.add_space(6.);
        heading(ui, lang.t(Key::home_tagline), 17.);
        ui.add_space(10.);
        dim(ui, lang.t(Key::home_web_line));
        ui.add_space(14.);
        if accent_button(ui, lang.t(Key::home_to_downloads)) {
            self.page = Page::Downloads;
        }
        ui.add_space(22.);
        card(ui, |ui| {
            small_heading(ui, lang.t(Key::home_fork_title));
            for key in [
                Key::home_fork_direct,
                Key::home_fork_x86,
                Key::home_fork_legacy,
            ] {
                ui.add_space(6.);
                bullet(ui, lang.t(key));
            }
        });
        ui.add_space(10.);
        card(ui, |ui| dim(ui, lang.t(Key::home_game_note)));
        ui.add_space(18.);
        links(ui);
    }

    fn downloads(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        if matches!(self.inner.borrow().releases, Releases::NotAsked) {
            start_releases(&self.inner);
        }
        heading(ui, lang.t(Key::downloads_title), 22.);
        dim(ui, lang.t(Key::downloads_line));
        ui.add_space(10.);
        let releases = match &self.inner.borrow().releases {
            Releases::NotAsked | Releases::Loading => {
                dim(ui, lang.t(Key::downloads_loading));
                return;
            }
            Releases::Failed(error) => {
                ui.colored_label(
                    color(glass::WARN),
                    format!("{}: {error}", lang.t(Key::downloads_failed)),
                );
                if error.contains("HTTP 403") {
                    ui.add_space(4.);
                    dim(ui, lang.t(Key::downloads_rate_limited));
                }
                if ui.button(lang.t(Key::downloads_retry)).clicked() {
                    self.inner.borrow_mut().releases = Releases::NotAsked;
                }
                return;
            }
            Releases::Loaded(releases) => releases.clone(),
        };
        if releases.is_empty() {
            dim(ui, lang.t(Key::downloads_none));
            return;
        }
        let recommended = recommended_asset();
        for release in &releases {
            ui.add_space(10.);
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    heading(ui, &release.name, 16.);
                    if release.prerelease {
                        ui.add_space(8.);
                        badge(ui, lang.t(Key::downloads_prerelease), glass::WARN);
                    }
                });
                dim(ui, &release.published);
                ui.add_space(8.);
                for asset in &release.assets {
                    asset_row(ui, lang, asset, recommended.as_deref());
                }
            });
        }
        ui.add_space(14.);
        if ui.button(lang.t(Key::releases_page)).clicked() {
            platform::open_url(&format!("https://github.com/{}/releases", pins::REPO));
        }
    }

    fn minecraft(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        heading(
            ui,
            format!("{} {}", lang.t(Key::mc_title), pins::MC_VERSION),
            22.,
        );
        dim(ui, lang.t(Key::mc_intro));
        ui.add_space(10.);
        card(ui, |ui| {
            pin_row(ui, lang, Key::mc_client, short(pins::MC_CLIENT_SHA1));
            pin_row(ui, lang, Key::mc_version_file, short(pins::MC_MANIFESTS[0]));
            let first_index = pins::MC_ASSET_INDEXES.first().copied().unwrap_or("");
            pin_row(ui, lang, Key::mc_asset_index, short(first_index));
        });
        ui.add_space(10.);
        dim(ui, lang.t(Key::mc_browser_line));

        // The folder.
        ui.add_space(16.);
        small_heading(ui, lang.t(Key::mc_pick_folder));
        match self.inner.borrow().folder.clone() {
            Folder::None => {
                dim(ui, lang.t(Key::mc_folder_none));
            }
            Folder::Unsupported => {
                ui.colored_label(color(glass::WARN), lang.t(Key::mc_folder_unsupported));
            }
            Folder::Failed(error) => {
                ui.colored_label(color(glass::DANGER), error);
            }
            Folder::Ready => {
                ui.colored_label(color(glass::LIVE), lang.t(Key::mc_folder_ready));
                ui.monospace(
                    egui::RichText::new(platform::folder_name())
                        .weak()
                        .color(TEXT),
                );
            }
        }
        ui.add_space(6.);
        let pick_label = match self.inner.borrow().folder {
            Folder::Ready => lang.t(Key::mc_pick_again),
            _ => lang.t(Key::mc_pick_folder),
        };
        if ui
            .add_enabled(platform::can_pick_folder(), egui::Button::new(pick_label))
            .clicked()
        {
            let inner = self.inner.clone();
            platform::spawn(async move {
                match platform::pick_folder().await {
                    Ok(platform::Picked::Chosen) => inner.borrow_mut().folder = Folder::Ready,
                    Ok(platform::Picked::Cancelled) => {}
                    Ok(platform::Picked::Unsupported) => {
                        inner.borrow_mut().folder = Folder::Unsupported
                    }
                    Err(error) => inner.borrow_mut().folder = Folder::Failed(error),
                }
            });
        }

        // Download directly.
        ui.add_space(16.);
        small_heading(ui, lang.t(Key::mc_download));
        let running = matches!(self.inner.borrow().download, Download::Running { .. });
        let ready = matches!(self.inner.borrow().folder, Folder::Ready);
        ui.horizontal(|ui| {
            let download = ui.add_enabled(
                ready && !running,
                egui::Button::new(lang.t(Key::mc_download)).fill(color(glass::ACCENT)),
            );
            if download.clicked() {
                start_download(self.inner.clone());
            }
            if running && ui.button(lang.t(Key::mc_cancel)).clicked() {
                self.inner.borrow_mut().cancel = true;
            }
        });
        ui.add_space(6.);
        self.download_state(ui);

        // Verify.
        ui.add_space(16.);
        small_heading(ui, lang.t(Key::mc_verify));
        let verifying = self.inner.borrow().verifying;
        let checked = self.inner.borrow().verify.is_some();
        let verify = ui.add_enabled(
            ready && !verifying,
            egui::Button::new(match checked {
                true => lang.t(Key::mc_verify_again),
                false => lang.t(Key::mc_verify),
            }),
        );
        if verify.clicked() {
            start_verify(self.inner.clone());
        }
        if verifying {
            dim(ui, lang.t(Key::mc_verifying));
        }
        if !ready {
            dim(ui, lang.t(Key::mc_verify_need_folder));
        }
        if let Some(result) = &self.inner.borrow().verify {
            ui.add_space(6.);
            card(ui, |ui| {
                outcome_row(ui, lang, Key::mc_client, result.client);
                outcome_row(ui, lang, Key::mc_version_file, result.version_file);
                outcome_row(ui, lang, Key::mc_asset_index, result.asset_index);
            });
        }
    }

    /// The download under way, or how the last one ended.
    fn download_state(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let state = self.inner.borrow();
        match &state.download {
            Download::Idle => {}
            Download::Running {
                done,
                total,
                bytes,
                total_bytes,
                current,
                log,
            } => {
                let fraction = if *total_bytes > 0 {
                    (*bytes as f32 / *total_bytes as f32).clamp(0., 1.)
                } else {
                    0.
                };
                ui.vertical(|ui| {
                    ui.label(format!(
                        "{} {} — {} {}/{}",
                        lang.t(Key::mc_downloading),
                        current,
                        lang.t(Key::mc_file_of_files),
                        done,
                        total
                    ));
                    ui.add(egui::ProgressBar::new(fraction).text(format!(
                        "{} / {}",
                        size_text(*bytes),
                        size_text(*total_bytes)
                    )));
                    ui.add_space(4.);
                    dim(
                        ui,
                        format!("{}/{} {}", done, total, lang.t(Key::mc_files_done)),
                    );
                    for line in log.iter().rev().take(4) {
                        dim(ui, line);
                    }
                });
            }
            Download::Done(lines) => {
                ui.vertical(|ui| {
                    ui.colored_label(color(glass::LIVE), lang.t(Key::mc_download_done));
                    for line in lines {
                        dim(ui, line);
                    }
                });
            }
            Download::Failed(error) => {
                let text = if error == CANCELLED {
                    lang.t(Key::mc_cancelled).to_string()
                } else {
                    error.clone()
                };
                ui.colored_label(color(glass::DANGER), text);
            }
        }
    }

    fn footer(ui: &mut egui::Ui, lang: Lang) {
        ui.add_space(24.);
        ui.separator();
        ui.add_space(6.);
        ui.label(
            egui::RichText::new(format!("Horizon {} · web · {}", pins::VERSION, pins::REPO))
                .small()
                .weak(),
        );
        dim(ui, lang.t(Key::footer_not_mojang));
    }
}

// ── The tasks ───────────────────────────────────────────────────────────────────────────

/// The fork's releases from GitHub (api.github.com allows a browser), as the desktop
/// launcher's own updates read them.
fn start_releases(inner: &Rc<RefCell<Inner>>) {
    inner.borrow_mut().releases = Releases::Loading;
    let inner = inner.clone();
    platform::spawn(async move {
        let url = format!(
            "https://api.github.com/repos/{}/releases?per_page=8",
            pins::REPO
        );
        let answer = platform::fetch_text(&url).await.and_then(|text| {
            serde_json::from_str::<Vec<GhRelease>>(&text)
                .map_err(|e| format!("GitHub's releases: {e}"))
        });
        let mut state = inner.borrow_mut();
        state.releases = match answer {
            Ok(list) => Releases::Loaded(
                list.into_iter()
                    .map(|release| Release {
                        name: release.name.unwrap_or_else(|| release.tag_name.clone()),
                        // The date, cut to its day.
                        published: release.published_at.chars().take(10).collect(),
                        prerelease: release.prerelease,
                        assets: release
                            .assets
                            .into_iter()
                            .filter(|asset| {
                                (asset.name.starts_with("horizon-")
                                    && (asset.name.ends_with(".zip")
                                        || asset.name.ends_with(".tar.gz")))
                                    || asset.name.ends_with(".exe")
                            })
                            .collect(),
                    })
                    .collect(),
            ),
            Err(error) => Releases::Failed(error),
        };
    });
}

/// The pinned version's files, as far as the browser may: the version file, the client jar,
/// the logging config, the asset index — every file checked against the sha1 the pin holds
/// or the version file lists, written whole, kept files skipped (a download stopped half way
/// goes on where it stopped).
fn start_download(inner: Rc<RefCell<Inner>>) {
    {
        let mut state = inner.borrow_mut();
        state.cancel = false;
        state.download = Download::Running {
            done: 0,
            total: 1,
            bytes: 0,
            total_bytes: 0,
            current: String::new(),
            log: Vec::new(),
        };
    }
    platform::spawn(async move {
        let result = run_download(&inner).await;
        let mut state = inner.borrow_mut();
        state.download = match result {
            Ok(lines) => Download::Done(lines),
            Err(error) => Download::Failed(error),
        };
    });
}

async fn run_download(inner: &Rc<RefCell<Inner>>) -> Result<Vec<String>, String> {
    let v = pins::MC_VERSION;
    let version_file = manifest::DownloadFile::version_json(v, pins::MC_MANIFESTS);
    // The version file: kept when it is one the pins accept (Mojang republished some).
    let json_text = match platform::folder_sha1(&version_file.path).await {
        Some(sha1) if pins::MC_MANIFESTS.contains(&sha1.as_str()) => {
            {
                let mut state = inner.borrow_mut();
                log(
                    &mut state,
                    format!("The {v} version file is already there, kept"),
                );
            }
            let bytes = platform::read_file(&version_file.path).await?;
            finish_file(inner, 0);
            String::from_utf8_lossy(&bytes).into_owned()
        }
        _ => {
            {
                let mut state = inner.borrow_mut();
                log(
                    &mut state,
                    format!("Downloading the {v} version file from Mojang"),
                );
            }
            let bytes = fetch_file(inner, &version_file, 0).await?;
            let sha1 = sha1_hex(&bytes);
            if !pins::MC_MANIFESTS.contains(&sha1.as_str()) {
                return Err(format!(
                    "Mojang's {v} version file is not the one Horizon pins (sha1 {sha1})"
                ));
            }
            platform::write_file(&version_file.path, &bytes).await?;
            finish_file(inner, 0);
            String::from_utf8_lossy(&bytes).into_owned()
        }
    };
    // The rest of the plan.
    let files = manifest::plan(&json_text, pins::MC_CLIENT_SHA1)?;
    {
        let mut state = inner.borrow_mut();
        if let Download::Running {
            total, total_bytes, ..
        } = &mut state.download
        {
            *total = 1 + files.len();
            *total_bytes = files.iter().map(|file| file.size.unwrap_or(0)).sum();
        }
    }
    let mut done_bytes = 0u64;
    for file in &files {
        if inner.borrow().cancel {
            return Err(CANCELLED.into());
        }
        if platform::folder_sha1(&file.path).await.as_deref() == Some(file.sha1.as_str()) {
            {
                let mut state = inner.borrow_mut();
                log(&mut state, format!("{} is already whole, kept", file.path));
                if let Download::Running { done, .. } = &mut state.download {
                    *done += 1;
                }
            }
            done_bytes += file.size.unwrap_or(0);
            continue;
        }
        let bytes = fetch_file(inner, file, done_bytes).await?;
        let sha1 = sha1_hex(&bytes);
        if sha1 != file.sha1 {
            return Err(format!(
                "{} is not the one Horizon pins (sha1 {sha1})",
                file.url
            ));
        }
        platform::write_file(&file.path, &bytes).await?;
        done_bytes += file.size.unwrap_or(bytes.len() as u64);
        finish_file(inner, done_bytes);
    }
    Ok(vec![format!(
        "Minecraft {v}'s version file, client jar and asset index are in the folder"
    )])
}

/// Fetches `file`, its arrival told into the running download's progress bar.
async fn fetch_file(
    inner: &Rc<RefCell<Inner>>,
    file: &manifest::DownloadFile,
    bytes_before: u64,
) -> Result<Vec<u8>, String> {
    {
        let mut state = inner.borrow_mut();
        let size = file.size.map(size_text).unwrap_or_default();
        log(
            &mut state,
            if size.is_empty() {
                format!("Downloading {}", file.path)
            } else {
                format!("Downloading {} ({size})", file.path)
            },
        );
        if let Download::Running { current, .. } = &mut state.download {
            *current = file.path.clone();
        }
    }
    let telling = inner.clone();
    let progress = move |loaded: u64, _total: u64| {
        let mut state = telling.borrow_mut();
        if let Download::Running { bytes, .. } = &mut state.download {
            *bytes = bytes_before + loaded;
        }
    };
    platform::fetch_bytes(&file.url, progress).await
}

/// Counts the file under way done.
fn finish_file(inner: &Rc<RefCell<Inner>>, bytes: u64) {
    let mut state = inner.borrow_mut();
    if let Download::Running {
        done,
        bytes: so_far,
        ..
    } = &mut state.download
    {
        *done += 1;
        *so_far = bytes;
    }
}

/// Reads the folder's three pinned files and compares each sha1 with the pin, as
/// launcher/src/sources.rs compares them.
fn start_verify(inner: Rc<RefCell<Inner>>) {
    inner.borrow_mut().verifying = true;
    platform::spawn(async move {
        let v = pins::MC_VERSION;
        let version_path = format!("versions/{v}/{v}.json");
        let version_file = match platform::folder_sha1(&version_path).await {
            Some(sha1) if pins::MC_MANIFESTS.contains(&sha1.as_str()) => Outcome::Matches,
            Some(_) => Outcome::Differs,
            None => Outcome::Missing,
        };
        let client = match platform::folder_sha1(&format!("versions/{v}/{v}.jar")).await {
            Some(sha1) if sha1 == pins::MC_CLIENT_SHA1 => Outcome::Matches,
            Some(_) => Outcome::Differs,
            None => Outcome::Missing,
        };
        // The asset index's id (34) comes from the version file itself.
        let asset_index = match version_file {
            Outcome::Matches => {
                let id = platform::read_file(&version_path)
                    .await
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                    .and_then(|json| json["assetIndex"]["id"].as_str().map(String::from))
                    .and_then(|id| manifest::relative(&id).ok().filter(|id| !id.is_empty()));
                match id {
                    Some(id) => {
                        match platform::folder_sha1(&format!("assets/indexes/{id}.json")).await {
                            Some(sha1) if pins::MC_ASSET_INDEXES.contains(&sha1.as_str()) => {
                                Outcome::Matches
                            }
                            Some(_) => Outcome::Differs,
                            None => Outcome::Missing,
                        }
                    }
                    None => Outcome::Missing,
                }
            }
            _ => Outcome::Missing,
        };
        let mut state = inner.borrow_mut();
        state.verifying = false;
        state.verify = Some(Verify {
            client,
            version_file,
            asset_index,
        });
    });
}

fn log(state: &mut Inner, line: String) {
    if let Download::Running { log, .. } = &mut state.download {
        log.push(line);
    }
}

fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

// ── The widgets, in the launcher's look ──────────────────────────────────────────────────

fn install_fonts_and_style(ctx: &egui::Context) {
    // The launcher's pages are dark; the light theme stays whatever the player set.
    ctx.set_theme(egui::ThemePreference::Dark);
    use horizon_theme::fonts as theme;
    let mut fonts = egui::FontDefinitions::default();
    let add = |fonts: &mut egui::FontDefinitions, name: &str, data: &'static [u8]| {
        fonts
            .font_data
            .insert(name.to_string(), egui::FontData::from_static(data).into());
    };
    add(&mut fonts, "Geist", theme::GEIST_REGULAR);
    add(&mut fonts, "Geist Medium", theme::GEIST_MEDIUM);
    add(&mut fonts, "Geist SemiBold", theme::GEIST_SEMIBOLD);
    add(&mut fonts, "Geist Mono", theme::GEIST_MONO);
    add(&mut fonts, "Pixelify", theme::PIXEL_BOLD);
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Geist".into());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "Geist Mono".into());
    // The wordmark's family, registered so the fallbacks stay behind it.
    fonts
        .families
        .entry(egui::FontFamily::Name("Pixelify".into()))
        .or_default()
        .insert(0, "Pixelify".into());
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = color(glass::PAGE);
    style.visuals.window_fill = color(glass::ON_LIGHT);
    style.visuals.hyperlink_color = color(glass::ACCENT);
    style.visuals.selection.bg_fill = color(glass::ACCENT);
    style.visuals.widgets.inactive.fg_stroke.color = TEXT;
    style.visuals.widgets.hovered.fg_stroke.color = TEXT;
    style.visuals.widgets.active.fg_stroke.color = TEXT;
    ctx.set_style_of(egui::Theme::Dark, style);
}

fn color(hex: u32) -> egui::Color32 {
    egui::Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn wordmark(ui: &mut egui::Ui, text: &str, size: f32) {
    ui.label(
        egui::RichText::new(text)
            .family(egui::FontFamily::Name("Pixelify".into()))
            .size(size)
            .color(color(glass::ACCENT))
            .strong(),
    );
}

fn heading(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>, size: f32) {
    ui.label(text.into().size(size).strong().color(TEXT));
}

fn small_heading(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) {
    ui.label(text.into().size(15.).strong().color(TEXT));
}

fn dim(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) {
    ui.label(text.into().color(egui::Color32::from_rgb(0xa8, 0xa1, 0xc4)));
}

fn bullet(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) {
    ui.horizontal(|ui| {
        ui.label(
            <egui::WidgetText as From<&str>>::from("·")
                .color(color(glass::ACCENT))
                .strong(),
        );
        dim(ui, text);
    });
}

fn card(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::default()
        .fill(color(glass::ON_LIGHT))
        .corner_radius(10.)
        .inner_margin(egui::Margin::same(14))
        .show(ui, contents);
}

fn accent_button(ui: &mut egui::Ui, text: &str) -> bool {
    let button = egui::Button::new(egui::RichText::new(text).color(egui::Color32::WHITE))
        .fill(color(glass::ACCENT));
    ui.add(button).clicked()
}

fn badge(ui: &mut egui::Ui, text: &str, hex: u32) {
    ui.label(
        egui::RichText::new(format!(" {text} "))
            .small()
            .background_color(color(hex).gamma_multiply(0.25))
            .color(color(hex))
            .strong(),
    );
}

fn hyperlink(ui: &mut egui::Ui, url: &str, text: &str) {
    ui.add(egui::Hyperlink::from_label_and_url(text, url));
}

fn links(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (text, url) in [
            ("playhorizon.dev", "https://playhorizon.dev"),
            ("Discord", "https://discord.gg/playhorizon"),
            ("GitHub", "https://github.com/horizon-rust/horizon"),
        ] {
            hyperlink(ui, url, text);
            ui.add_space(10.);
        }
    });
}

/// One asset of a release: the name and size, a *this device* badge when the browser's user
/// agent points at it, and the download link.
fn asset_row(ui: &mut egui::Ui, lang: Lang, asset: &GhAsset, recommended: Option<&str>) {
    ui.horizontal(|ui| {
        if recommended == Some(asset.name.as_str()) {
            badge(ui, lang.t(Key::downloads_this_device), glass::LIVE);
            ui.add_space(6.);
        }
        ui.monospace(egui::RichText::new(&asset.name).weak().color(TEXT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            hyperlink(ui, &asset.browser_download_url, lang.t(Key::downloads_get));
            ui.label(egui::RichText::new(size_text(asset.size)).weak());
        });
    });
}

fn pin_row(ui: &mut egui::Ui, lang: Lang, key: Key, short: impl Into<egui::WidgetText>) {
    ui.horizontal(|ui| {
        dim(ui, lang.t(key));
        ui.label(short.into().monospace().weak().color(TEXT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let full = match key {
                Key::mc_client => pins::MC_CLIENT_SHA1,
                Key::mc_version_file => pins::MC_MANIFESTS[0],
                _ => pins::MC_ASSET_INDEXES.first().copied().unwrap_or(""),
            };
            ui.monospace(egui::RichText::new(full).small().weak());
        });
    });
}

/// A file's check outcome: a coloured dot (drawn, not a glyph, so no font decides it) and the
/// verdict beside the file's name.
fn outcome_row(ui: &mut egui::Ui, lang: Lang, key: Key, outcome: Outcome) {
    ui.horizontal(|ui| {
        let (hex, text) = match outcome {
            Outcome::Matches => (glass::LIVE, lang.t(Key::mc_matches)),
            Outcome::Differs => (glass::DANGER, lang.t(Key::mc_differs)),
            Outcome::Missing => (glass::WARN, lang.t(Key::mc_missing)),
        };
        dot(ui, hex);
        dim(ui, lang.t(key));
        ui.label(egui::RichText::new(text).color(color(hex)).small());
    });
}

/// A small coloured dot, the launcher's marks without a font.
fn dot(ui: &mut egui::Ui, hex: u32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12., 12.), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4., color(hex));
}

fn short(sha1: &str) -> String {
    sha1.chars().take(7).collect()
}

fn size_text(size: u64) -> String {
    const MB: f64 = 1024. * 1024.;
    let mb = size as f64 / MB;
    if mb >= 1. {
        format!("{mb:.1} MB")
    } else {
        format!("{} KB", size / 1024)
    }
}

/// The launcher asset this device most likely wants, from the browser's user agent: a 64-bit
/// browser on Windows says Win64 (a 32-bit one on any Windows says neither, and the x86 build
/// runs everywhere).
fn recommended_asset() -> Option<String> {
    let agent = platform::user_agent().to_ascii_lowercase();
    let is_64_bit = agent.contains("win64") || agent.contains("x64") || agent.contains("wow64");
    let is_arm = agent.contains("arm");
    if agent.contains("windows") {
        Some(match (is_64_bit, is_arm) {
            (true, false) => "horizon-launcher-windows-x64.zip".into(),
            (false, false) => "horizon-launcher-windows-x86.zip".into(),
            (_, true) => "horizon-launcher-windows-arm64.zip".into(),
        })
    } else if agent.contains("linux") {
        Some("horizon-launcher-linux-x86_64.tar.gz".into())
    } else if agent.contains("macintosh") {
        Some(match agent.contains("intel") {
            true => "horizon-launcher-macos-x64.zip".into(),
            false => "horizon-launcher-macos-arm64.zip".into(),
        })
    } else {
        None
    }
}

// ── eframe: the browser build starts on index.html's canvas; a native run previews the pages

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The frames keep coming while a task is under way, so a download's progress bar moves.
        if self.inner.borrow().busy() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let lang = self.lang;
        egui::Panel::top("top").show(ui, |ui| {
            ui.add_space(10.);
            ui.horizontal(|ui| {
                wordmark(ui, "HORIZON", 24.);
                ui.add_space(18.);
                for (page, key) in [
                    (Page::Home, Key::page_home),
                    (Page::Downloads, Key::page_downloads),
                    (Page::Minecraft, Key::page_minecraft),
                ] {
                    ui.selectable_value(&mut self.page, page, lang.t(key));
                    ui.add_space(4.);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let other = lang.other();
                    if ui
                        .small_button(other.code().to_ascii_uppercase())
                        .on_hover_text("Bahasa / Language")
                        .clicked()
                    {
                        self.lang = other;
                    }
                    ui.label(
                        egui::RichText::new(format!("v{}", pins::VERSION))
                            .small()
                            .weak(),
                    );
                });
            });
            ui.add_space(6.);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(6.);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_max_width(760.);
                match self.page {
                    Page::Home => self.home(ui),
                    Page::Downloads => self.downloads(ui),
                    Page::Minecraft => self.minecraft(ui),
                }
                App::footer(ui, lang);
            });
        });
    }
}

/// The native development run: the pages, every browser power answering a note.
#[cfg(not(target_arch = "wasm32"))]
pub fn run() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([880., 640.])
            .with_min_inner_size([520., 420.]),
        ..Default::default()
    };
    eframe::run_native(
        "Horizon web",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

/// The browser build: index.html's `<canvas id="horizon">`, filled once wasm loads. The runner
/// installs its own panic handler.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn web_start() -> Result<(), JsValue> {
    let canvas = eframe::web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("horizon"))
        .and_then(|element| {
            element
                .dyn_into::<eframe::web_sys::HtmlCanvasElement>()
                .ok()
        })
        .expect("index.html holds the canvas #horizon");
    let options = eframe::WebOptions::default();
    wasm_bindgen_futures::spawn_local(async move {
        let runner = eframe::WebRunner::new();
        if let Err(error) = runner
            .start(canvas, options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
            .await
        {
            eprintln!("the web launcher could not start: {error:?}");
        }
    });
    Ok(())
}
