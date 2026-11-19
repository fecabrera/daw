//! Versioned application themes and bounded discovery outside the UI thread.
mod document;

use crate::{
    settings::{AppSettings, ColorTheme},
    theme::{self, Palette},
};
use document::{Document, validate_contrast};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{OnceLock, mpsc},
};

const MAX_ENTRIES: usize = 512;
const MAX_BYTES: usize = 32 * 1024;
pub(super) const LEGACY_MIDNIGHT_ID: &str = "user.midnight";

pub(super) fn deserialize_custom_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value
        .as_str()
        .filter(|id| document::valid_custom_id(id))
        .map(str::to_owned))
}

fn template(theme: ColorTheme) -> &'static str {
    match theme {
        ColorTheme::Dark => include_str!("../assets/themes/dark.json"),
        ColorTheme::Light => include_str!("../assets/themes/light.json"),
        ColorTheme::Mist => include_str!("../assets/themes/mist.json"),
        ColorTheme::Midnight => include_str!("../assets/themes/midnight.json"),
    }
}

fn mist_from_light(light: Palette) -> Palette {
    Palette {
        missing: theme::MIST.missing,
        warning: theme::MIST.warning,
        error: theme::MIST.error,
        meter: theme::MIST.meter,
        ..light
    }
}

struct Stock {
    colors: [Palette; ColorTheme::ALL.len()],
    mist: Option<Document>,
    issues: Vec<String>,
}

impl Stock {
    fn get() -> &'static Self {
        static STOCK: OnceLock<Stock> = OnceLock::new();
        STOCK.get_or_init(|| {
            let mut issues = Vec::new();
            let mut colors = ColorTheme::ALL.map(|base| match base {
                ColorTheme::Dark | ColorTheme::Midnight => theme::DARK,
                ColorTheme::Light => theme::LIGHT,
                ColorTheme::Mist => theme::MIST,
            });
            let mut mist = None;
            for base in ColorTheme::ALL {
                let index = base.index();
                let input = match base {
                    ColorTheme::Mist => mist_from_light(colors[ColorTheme::Light.index()]),
                    _ => colors[index],
                };
                let result = Document::parse(template(base).as_bytes()).and_then(|document| {
                    if document.id != base.id() {
                        return Err(format!("Expected theme ID {}", base.id()));
                    }
                    validate_contrast(document.overlay(input)).map(|colors| (document, colors))
                });
                match result {
                    Ok((document, resolved)) => {
                        colors[index] = resolved;
                        if base == ColorTheme::Mist {
                            mist = Some(document);
                        }
                    }
                    Err(error) => {
                        issues.push(format!("Embedded {}: {error}", base.name()));
                    }
                }
            }
            Self {
                colors,
                mist,
                issues,
            }
        })
    }
}

pub(super) fn stock_palette(base: ColorTheme) -> Palette {
    Stock::get().colors[base.index()]
}

pub(super) struct Entry {
    pub id: String,
    pub name: String,
    pub base: ColorTheme,
    colors: Palette,
}

type UserThemes = BTreeMap<String, Vec<(PathBuf, Document)>>;

struct Catalog {
    entries: Vec<Entry>,
    issues: Vec<String>,
}

impl Catalog {
    fn resolve(mut user: UserThemes, mut issues: Vec<String>) -> Self {
        let stock = Stock::get();
        issues.extend(stock.issues.iter().cloned());
        let mut entries: Vec<Entry> = Vec::new();
        for base in ColorTheme::ALL {
            let fallback = stock.colors[base.index()];
            // Mist overrides the effective Light palette. Its inherited roles
            // follow Light edits, including when a user also has a Mist file.
            let inherited = match (base, &stock.mist) {
                (ColorTheme::Mist, Some(mist)) => {
                    mist.overlay(mist_from_light(entries[ColorTheme::Light.index()].colors))
                }
                _ => fallback,
            };
            let override_colors = user
                .remove(base.id())
                .and_then(|files| resolve_files(files, |_| inherited, &mut issues))
                .map(|(_, colors)| colors);
            let colors = override_colors.unwrap_or_else(|| {
                validate_contrast(inherited).unwrap_or_else(|error| {
                    issues.push(format!(
                        "Mist cannot inherit Light: {error} Using stock Mist."
                    ));
                    fallback
                })
            });
            entries.push(Entry {
                id: base.id().into(),
                name: base.name().into(),
                base,
                colors,
            });
        }
        let mut custom = user
            .into_values()
            .filter_map(|files| {
                let (document, colors) =
                    resolve_files(files, |base| stock.colors[base.index()], &mut issues)?;
                Some(Entry {
                    id: document.id,
                    name: document.name,
                    base: document.base,
                    colors,
                })
            })
            .collect::<Vec<_>>();
        custom.sort_by_cached_key(|entry| (entry.name.to_lowercase(), entry.id.clone()));
        entries.extend(custom);
        issues.sort();
        Self { entries, issues }
    }

    fn scan(path: &Path) -> Result<Self, String> {
        let mut themes = UserThemes::new();
        let mut issues = Vec::new();
        let directory = match fs::read_dir(path) {
            Ok(directory) => directory,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Self::resolve(themes, issues));
            }
            Err(error) => return Err(path_error(path, error)),
        };
        for (index, entry) in directory.enumerate() {
            if index == MAX_ENTRIES {
                return Err(path_error(
                    path,
                    format!("Use at most {MAX_ENTRIES} folder entries"),
                ));
            }
            let entry = entry.map_err(|error| path_error(path, error))?;
            let file = entry.path();
            if !file
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            {
                continue;
            }
            match entry.file_type() {
                Ok(kind) if kind.is_file() => {}
                Ok(_) => continue,
                Err(error) => {
                    issues.push(path_error(&file, error));
                    continue;
                }
            }
            match read_theme(&file) {
                Ok(document) => themes
                    .entry(document.id.clone())
                    .or_default()
                    .push((file, document)),
                Err(error) => issues.push(path_error(&file, error)),
            }
        }
        Ok(Self::resolve(themes, issues))
    }
}

fn resolve_files(
    mut files: Vec<(PathBuf, Document)>,
    base_palette: impl Fn(ColorTheme) -> Palette,
    issues: &mut Vec<String>,
) -> Option<(Document, Palette)> {
    files.sort_by(|(left, _), (right, _)| left.cmp(right));
    let mut valid = Vec::new();
    for (path, document) in files {
        let colors = document.overlay(base_palette(document.base));
        match validate_contrast(colors) {
            Ok(colors) => valid.push((path, document, colors)),
            Err(error) => issues.push(path_error(&path, error)),
        }
    }
    if valid.len() > 1 {
        let paths = valid
            .iter()
            .map(|(path, _, _)| path.display().to_string())
            .collect::<Vec<_>>();
        issues.push(format!(
            "Duplicate theme ID {}: {}",
            valid[0].1.id,
            paths.join(", ")
        ));
        return None;
    }
    valid.pop().map(|(_, document, colors)| (document, colors))
}

fn path_error(path: &Path, error: impl std::fmt::Display) -> String {
    format!("{}: {error}", path.display())
}

fn read_theme(path: &Path) -> Result<Document, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes))
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err(format!("Theme exceeds {MAX_BYTES} bytes"));
    }
    Document::parse(&bytes)
}

#[derive(Clone, Copy)]
pub(super) enum Action {
    Reload,
    OpenFolder,
}

type FolderOpener = fn(&Path) -> Result<(), String>;

pub(super) struct Runtime {
    pub directory: Option<PathBuf>,
    catalog: Catalog,
    pending: Option<mpsc::Receiver<Result<Option<Catalog>, String>>>,
    open_folder: Option<FolderOpener>,
    error: Option<String>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            directory: None,
            catalog: Catalog::resolve(UserThemes::new(), Vec::new()),
            pending: None,
            open_folder: None,
            error: None,
        }
    }
}

impl Runtime {
    pub fn set_directory(
        &mut self,
        ctx: &egui::Context,
        directory: Option<PathBuf>,
        open_folder: FolderOpener,
    ) {
        self.pending = None;
        self.directory = directory.filter(|path| path.is_absolute());
        self.open_folder = Some(open_folder);
        self.start(ctx, Action::Reload);
    }

    pub fn entries(&self) -> &[Entry] {
        &self.catalog.entries
    }

    pub fn active(&self, settings: &AppSettings) -> &Entry {
        settings
            .custom_theme_id
            .as_deref()
            .and_then(|id| self.entries().iter().find(|entry| entry.id == id))
            .unwrap_or(&self.entries()[settings.theme.index()])
    }

    pub fn apply(&self, ctx: &egui::Context, settings: &AppSettings) {
        let entry = self.active(settings);
        theme::apply_resolved(ctx, entry.base, entry.colors, settings.accent);
        ctx.request_repaint();
    }

    pub fn sync_settings_surface(&self, ctx: &egui::Context, settings: &AppSettings) {
        let entry = self.active(settings);
        let palette = (entry.base, entry.colors, settings.accent);
        let id = egui::Id::new(("settings-surface-palette", ctx.viewport_id()));
        if ctx.data(|data| data.get_temp(id)) != Some(palette) {
            ctx.data_mut(|data| data.insert_temp(id, palette));
            self.apply(ctx, settings);
        }
    }

    pub fn status(&self) -> impl Iterator<Item = &str> {
        self.error
            .iter()
            .chain(&self.catalog.issues)
            .map(String::as_str)
    }

    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    pub fn start(&mut self, ctx: &egui::Context, action: Action) {
        let Some(directory) = self.directory.clone().filter(|_| self.pending.is_none()) else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        let open_folder = self.open_folder;
        match std::thread::Builder::new()
            .name("theme-discovery".into())
            .spawn(move || {
                let result = match action {
                    Action::Reload => Catalog::scan(&directory).map(Some),
                    Action::OpenFolder => {
                        let open = || {
                            let open_folder = open_folder.ok_or("Folder opener unavailable")?;
                            fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
                            open_folder(&directory)
                        };
                        open()
                            .map(|()| None)
                            .map_err(|error| path_error(&directory, error))
                    }
                };
                let _ = sender.send(result);
                ctx.request_repaint();
            }) {
            Ok(_) => self.pending = Some(receiver),
            Err(error) => self.error = Some(format!("Cannot start theme discovery: {error}")),
        }
    }

    pub fn poll(&mut self, ctx: &egui::Context, settings: &AppSettings) {
        let Some(result) = self.pending.as_ref().and_then(|receiver| {
            crate::worker_result::poll(receiver, || "Theme discovery stopped".into())
        }) else {
            return;
        };
        self.pending = None;
        match result {
            Ok(Some(catalog)) => {
                self.catalog = catalog;
                self.error = None;
                self.apply(ctx, settings);
            }
            Ok(None) => self.error = None,
            Err(error) => self.error = Some(error),
        }
    }
}

#[cfg(test)]
mod tests;
