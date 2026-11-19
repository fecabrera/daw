//! Versioned application themes and bounded discovery outside the UI thread.
mod document;

use crate::{
    settings::{AppSettings, ColorTheme},
    theme::{self, Palette},
};
use document::{Document, validate_contrast};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{OnceLock, mpsc},
};

const MAX_ENTRIES: usize = 512;
const MAX_BYTES: usize = 32 * 1024;

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
    colors: [Palette; 3],
    mist: Option<Document>,
    templates: [String; 3],
    issues: Vec<String>,
}

impl Stock {
    fn get() -> &'static Self {
        static STOCK: OnceLock<Stock> = OnceLock::new();
        STOCK.get_or_init(|| {
            let mut issues = Vec::new();
            let mut colors = [theme::DARK, theme::LIGHT, theme::MIST];
            let mut templates = ColorTheme::ALL.map(|base| template(base).to_owned());
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
                        templates[index] = document::export_stock(base, colors[index], input);
                    }
                }
            }
            Self {
                colors,
                mist,
                templates,
                issues,
            }
        })
    }
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
        discover(path).map(|(themes, issues)| Self::resolve(themes, issues))
    }
}

fn discover(path: &Path) -> Result<(UserThemes, Vec<String>), String> {
    let mut themes = UserThemes::new();
    let mut issues = Vec::new();
    let directory = match fs::read_dir(path) {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((themes, issues)),
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
    Ok((themes, issues))
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

fn create_overrides(directory: &Path) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|error| path_error(directory, error))?;
    let (existing, _) = discover(directory)?;
    for base in ColorTheme::ALL {
        if existing.contains_key(base.id()) {
            continue;
        }
        let path = directory.join(format!("{}.json", base.id()));
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(path_error(&path, error)),
        };
        if let Err(error) = file.write_all(Stock::get().templates[base.index()].as_bytes()) {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(path_error(&path, error));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) enum Action {
    Reload,
    CreateOverrides,
}

pub(super) struct Runtime {
    pub directory: Option<PathBuf>,
    catalog: Catalog,
    pending: Option<mpsc::Receiver<Result<Catalog, String>>>,
    error: Option<String>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            directory: None,
            catalog: Catalog::resolve(UserThemes::new(), Vec::new()),
            pending: None,
            error: None,
        }
    }
}

impl Runtime {
    pub fn set_directory(&mut self, ctx: &egui::Context, directory: Option<PathBuf>) {
        self.pending = None;
        self.directory = directory.filter(|path| path.is_absolute());
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

    pub fn copy_stock(&self, ctx: &egui::Context, settings: &AppSettings) {
        ctx.copy_text(Stock::get().templates[self.active(settings).base.index()].clone());
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
        match std::thread::Builder::new()
            .name("theme-discovery".into())
            .spawn(move || {
                let result = match action {
                    Action::Reload => Catalog::scan(&directory),
                    Action::CreateOverrides => {
                        create_overrides(&directory).and_then(|()| Catalog::scan(&directory))
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
            Ok(catalog) => {
                self.catalog = catalog;
                self.error = None;
                self.apply(ctx, settings);
            }
            Err(error) => self.error = Some(error),
        }
    }
}

#[cfg(test)]
mod tests;
