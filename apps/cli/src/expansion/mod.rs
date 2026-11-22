//! Additional saved-project command families. All mutations share one guarded save.
use crate::{report, tracks};
use clap::{Arg, ArgAction, ArgGroup, ArgMatches, Command, value_parser};
use daw_core::{Clip, Edit, Id, Project, Track};
use daw_project::{Error, ProjectWriteGuard, Result, Session};
use serde_json::{Value, json};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

mod arrangement;
mod patterns;
mod saved;
mod units;
pub(crate) use units::identity_key;
mod routes;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Access {
    Local,
    Inspect,
    Edit,
    Prepare,
    Special,
}

#[derive(Clone, Copy)]
enum Kind {
    Text,
    Name,
    Selector,
    Id,
    File,
    Frame,
    Count,
    Index,
    Signed,
    Small,
    Note,
    Wide,
    Decimal,
    Boolean,
    Flag,
    Choice(&'static [&'static str]),
}

#[derive(Clone, Copy)]
struct Param {
    name: &'static str,
    kind: Kind,
    required: bool,
    many: bool,
}

impl Param {
    fn argument(self) -> Arg {
        let arg = Arg::new(self.name).long(self.name).required(self.required);
        let arg = match self.kind {
            Kind::Text if self.name == "value" => {
                arg.value_name("VALUE").allow_negative_numbers(true)
            }
            Kind::Text => match self.name {
                "owner" | "to-owner" => arg
                    .value_name("OWNER")
                    .help("Typed saved owner. See the CLI catalog for owner forms."),
                "parameter" => arg
                    .value_name("KEY")
                    .help("Parameter key reported by the unit or plugin listing."),
                "plugin-id" => arg
                    .value_name("KEY")
                    .help("Installed plugin key from plugin.catalog.list."),
                "before" => arg
                    .value_name("ANCHOR")
                    .help("Insertion anchor. Use end to append when supported."),
                "parent" => arg.value_name("GROUP_OR_MASTER"),
                "source" => arg.value_name("LAYER_UUID:PROVIDER_UUID"),
                "destination" => arg.value_name("UNIT_UUID:PARAMETER[:TARGET_UUID]"),
                "color" => arg.value_name("#RRGGBB"),
                _ => arg.value_name("KEY"),
            },
            Kind::Name => arg.value_name("NAME").value_parser(tracks::parse_name),
            Kind::Selector => arg
                .value_name("id:UUID/name:EXACT")
                .value_parser(tracks::parse_selector),
            Kind::Id => arg.value_name("UUID").value_parser(value_parser!(Id)),
            Kind::File => arg.value_name("PATH").value_parser(value_parser!(PathBuf)),
            Kind::Frame => arg.value_name("FRAME").value_parser(value_parser!(u64)),
            Kind::Count => arg.value_name("NUMBER").value_parser(value_parser!(u32)),
            Kind::Index => arg.value_name("INDEX").value_parser(value_parser!(usize)),
            Kind::Signed => arg
                .value_name("NUMBER")
                .allow_negative_numbers(true)
                .value_parser(value_parser!(i64)),
            Kind::Small => arg
                .value_name("NUMBER")
                .allow_negative_numbers(true)
                .value_parser(value_parser!(i8)),
            Kind::Wide => arg.value_name("NUMBER").value_parser(value_parser!(u16)),
            Kind::Note => arg.value_name("NUMBER").value_parser(value_parser!(u8)),
            Kind::Decimal => arg
                .value_name("NUMBER")
                .allow_negative_numbers(true)
                .value_parser(value_parser!(f32)),
            Kind::Boolean => arg
                .value_name("true|false")
                .value_parser(value_parser!(bool)),
            Kind::Flag => return arg.action(ArgAction::SetTrue),
            Kind::Choice(values) => {
                arg.value_name("VALUE")
                    .value_parser(clap::builder::PossibleValuesParser::new(
                        values.iter().copied(),
                    ))
            }
        };
        if self.many {
            arg.action(ArgAction::Append).num_args(1..)
        } else {
            arg
        }
    }
}

pub(super) struct Route {
    path: &'static str,
    about: &'static str,
    access: Access,
    params: &'static [Param],
    handler: fn(&str, &ArgMatches, &mut Session) -> Result<Value>,
}

impl Route {
    fn command(&self) -> Command {
        let command = Command::new(self.path)
            .about(self.about)
            .args(self.params.iter().map(|arg| arg.argument()));
        let alternatives = match self.path {
            "clip.color.set" => Some(["color", "inherit"]),
            "beat.subtrack.stop.set" => Some(["target", "clear"]),
            "plugin.instance.sidechain.set" => Some(["source-track", "clear"]),
            _ => None,
        };
        match alternatives {
            Some(options) => command.group(ArgGroup::new("choice").args(options).required(true)),
            None => command,
        }
    }

    pub(super) fn is_local(&self) -> bool {
        self.access == Access::Local
    }

    pub(super) fn execute(
        &self,
        args: &ArgMatches,
        folder: Option<&Path>,
    ) -> Result<report::Outcome> {
        super::initialize(self.access == Access::Prepare || self.access == Access::Special);
        if self.access == Access::Special {
            return saved::special(self.path, args, folder.expect("validated context"));
        }
        let mut loaded;
        let (session, saved) = match self.access {
            Access::Inspect => {
                let folder = folder.expect("validated context");
                loaded = Session::default();
                loaded.project = daw_project::inspect_project(folder)?;
                loaded.folder = Some(daw_project::absolute(folder)?);
                (&mut loaded, None)
            }
            Access::Edit | Access::Prepare => {
                let guard = ProjectWriteGuard::acquire(folder.expect("validated context"))?;
                let (document, revision) = guard.open_document()?;
                loaded = document.into_editor_session();
                if self.access == Access::Prepare {
                    prepare(&mut loaded)?;
                }
                (&mut loaded, Some((guard, revision)))
            }
            Access::Local => {
                loaded = Session::default();
                (&mut loaded, None)
            }
            Access::Special => unreachable!(),
        };
        let result = (self.handler)(self.path, args, session)?;
        if let Some((guard, revision)) = saved {
            session.project.validate().map_err(error)?;
            let saved = session
                .save_snapshot(None)?
                .save_guarded(&guard, &revision)?;
            for warning in &saved.session.warnings {
                report::warning(warning);
            }
            Ok(committed(result))
        } else {
            Ok(read(result))
        }
    }
}

pub(super) fn register(registry: Command) -> Command {
    registry.subcommands(routes::ROUTES.iter().map(Route::command))
}

pub(super) fn route(path: &str) -> Option<&'static Route> {
    routes::ROUTES.iter().find(|route| route.path == path)
}

fn read(value: Value) -> report::Outcome {
    report::Outcome::read(format!("{value:#}\n"), value)
}

fn committed(value: Value) -> report::Outcome {
    report::Outcome::committed(format!("{value:#}\n"), value)
}

fn error(e: impl Display) -> Error {
    Error(e.to_string())
}

fn usage(message: &str) -> ! {
    super::canonical::usage(message)
}

fn get<'a, T: Clone + Send + Sync + 'static>(args: &'a ArgMatches, key: &str) -> &'a T {
    args.get_one(key).expect("registered required argument")
}

fn opt<'a, T: Clone + Send + Sync + 'static>(args: &'a ArgMatches, key: &str) -> Option<&'a T> {
    args.try_get_one(key).ok().flatten()
}

fn raw<'a>(args: &'a ArgMatches, key: &str) -> &'a str {
    get::<String>(args, key)
}

fn items<'a, T: Clone + Send + Sync + 'static>(
    args: &'a ArgMatches,
    key: &str,
) -> impl Iterator<Item = &'a T> {
    args.get_many::<T>(key).into_iter().flatten()
}

fn update<T: Copy + Send + Sync + 'static>(args: &ArgMatches, key: &str, field: &mut T) -> bool {
    if let Some(value) = opt(args, key) {
        *field = *value;
        true
    } else {
        false
    }
}

fn changes(present: bool) {
    if !present {
        usage("Supply at least one setting to change");
    }
}

fn select<'a, T>(
    entries: impl IntoIterator<Item = &'a T>,
    selector: &tracks::Selector,
    key: impl Fn(&T) -> (Id, &str),
) -> Result<&'a T> {
    let matches: Vec<_> = entries
        .into_iter()
        .filter(|entry| {
            let (id, name) = key(entry);
            match selector {
                tracks::Selector::Id(wanted) => id == *wanted,
                tracks::Selector::Name(wanted) => name == wanted,
            }
        })
        .collect();
    match matches.as_slice() {
        [only] => Ok(*only),
        [] => Err(error("target-not-found: selector has no match")),
        _ => Err(error(format!(
            "ambiguous-target: choose an ID from {}",
            matches
                .iter()
                .map(|entry| key(entry).0.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn track(project: &Project, selector: &tracks::Selector) -> Result<Id> {
    select(&project.tracks, selector, |t| (t.id, &t.name)).map(|t| t.id)
}

fn track_row(project: &Project, id: Id) -> &Track {
    project
        .tracks
        .iter()
        .find(|entry| entry.id == id)
        .expect("resolved track")
}

fn track_ids(project: &Project, args: &ArgMatches, key: &str) -> Result<Vec<Id>> {
    resolve_ids(items(args, key).map(|selector| track(project, selector)))
}

fn clip<'a>(
    project: &'a Project,
    selector: &tracks::Selector,
    owner: Option<Id>,
) -> Result<&'a Clip> {
    select(
        project
            .tracks
            .iter()
            .filter(|track| owner.is_none_or(|id| id == track.id))
            .flat_map(|track| &track.clips),
        selector,
        |clip| (clip.id, &clip.name),
    )
}

fn clip_owner(project: &Project, id: Id) -> &Track {
    project
        .tracks
        .iter()
        .find(|track| track.clips.iter().any(|clip| clip.id == id))
        .expect("resolved clip")
}

fn clip_row(project: &Project, id: Id) -> &Clip {
    clip_owner(project, id)
        .clips
        .iter()
        .find(|clip| clip.id == id)
        .expect("resolved clip")
}

fn clip_ids(project: &Project, args: &ArgMatches, key: &str) -> Result<Vec<Id>> {
    resolve_ids(items(args, key).map(|selector| Ok(clip(project, selector, None)?.id)))
}

fn resolve_ids(ids: impl Iterator<Item = Result<Id>>) -> Result<Vec<Id>> {
    let mut resolved = Vec::new();
    for id in ids {
        let id = id?;
        if !resolved.contains(&id) {
            resolved.push(id);
        }
    }
    Ok(resolved)
}

fn edit(session: &mut Session, edit: Edit) -> Result<()> {
    session.project.edit(edit).map_err(error)
}

fn prepare(session: &mut Session) -> Result<()> {
    session.prepare_project_audio(&|| false, &|_, _| {})
}

fn prepare_edit(session: &mut Session, edit: Edit) -> Result<()> {
    *session = session.with_edit(edit)?;
    Ok(())
}

fn color(text: &str) -> daw_core::RgbColor {
    let rgb = text
        .strip_prefix('#')
        .filter(|digits| digits.len() == 6)
        .and_then(|digits| u32::from_str_radix(digits, 16).ok())
        .unwrap_or_else(|| usage("Use a color of the form #RRGGBB"));
    daw_core::RgbColor {
        r: (rgb >> 16) as u8,
        g: (rgb >> 8) as u8,
        b: rgb as u8,
    }
}
