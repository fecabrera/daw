# Namespaced DAW CLI

Use one OS-native project folder argument before the command when the operation
needs a saved project. Project-free commands list machine-local plugins or built-in capabilities.
Match the full dotted path as one token. A path has two or more segments, with
no fixed depth limit. Each segment matches `[a-z][a-z0-9]*(-[a-z0-9]+)*`.
Registered leaves are unique and do not serve as namespace prefixes.

## Discovery and compatibility

Generate namespace listings from the typed Clap leaves. A bare namespace or
`PREFIX --help` lists its direct children. Each executable command supports
`COMMAND --help` and `help COMMAND`, including when its required arguments
are absent. Help exits successfully before project, device, cache or registry
access. Only shipped commands appear in discovery.

Keep the five legacy routes and their argument placement, defaults, outputs
and exit codes. Dispatch the private embedded scanner protocol before public
command lookup. It has no public registration. The [CLI guide](../../docs/cli/README.md)
links to the [complete command catalog](../../docs/cli/catalog.md) and migration examples.

## Targeting and results

Resolve `id:UUID` and `name:EXACT` in the loaded saved project. Singleton
operations reject missing and ambiguous names. List by name returns all
exact matches in saved order. New track names preserve significant whitespace
and reject blank text and control characters. A same-name rename skips save.
Use the shared core constructors and track name edit to preserve stable IDs and
group, routing and clip state.

Invocation-level `--json` selects a versioned object with `schema_version`,
`command` and a command-specific `result`. It never prints the project manifest
or the legacy raw plugin registry as the public result. Warnings and errors go
to stderr. Path strings are display values. Exit codes are 0 for success, 1
for operation errors and 2 for invalid usage. A confirmed commit stays
successful if writing its receipt fails.

## Storage and publication

Project mutations use a cooperative, nonblocking lock on the reserved
`.daw-cli-write.lock` entry in the physical project folder. Retain the entry
after release and verify its identity and the project root. Project mutations
capture the exact manifest bytes used to load the document and recheck the
revision before atomic publication. Collection performs its own verified
preflight and revision checks under the same lock. Blocked preflight and dry
runs leave project files untouched. The guard coordinates upgraded CLI
writers. External writers and the desktop can bypass it.

Export takes an explicit output file and uses the same prepared, hosted-aware
renderer as the legacy route. Confirm that the destination can be safely
published before staging and recheck it after encoding. Protect the manifest,
file-backed sources, saved plugin state and the reserved lock path through
canonical paths and existing file identity. Use the shared atomic
no-replacement publisher by default. An explicit overwrite permits replacing an
eligible output only. Existing user paths and output survive a confirmed
prepublication failure.

[Track creation](cli-track-creation.md) and [source collection](source-bundling-cli.md)
document the legacy routes.


## Extended operations

All catalog writes construct one validated saved-project candidate. Metadata
inspection uses the validated manifest without preparing audio or hosting
plugins. Rendering uses the same hosted broker as mix export. Range export
replays from the origin before writing the exclusive interval. Stem export
uses the shared bounce capture and publishes a completed directory without
replacement. Edit batches accept only the documented version 1 operations and
publish one guarded save. The recovery commands inspect verified generations
from inactive owners and hold the recovery lease while copying a generation.
