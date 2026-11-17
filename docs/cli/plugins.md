# Inspect installed plugins

Use `daw-cli plugins` to discover VST3 and CLAP plugin descriptors. This command
does not add plugins to a project. Use the [desktop plugin browser](../plugins.md)
to install units. `daw-cli render` processes plugins already saved in a project.

```sh
daw-cli plugins
daw-cli plugins --json
daw-cli plugins --cached
daw-cli plugins --retry-failed
daw-cli plugins --rescan
daw-cli plugins --clear-cache
```

The command searches the platform's user and system plugin folders, plus
CLAP_PATH. Default installations need no path configuration. It keeps a local
catalog, lists failed modules as unavailable and keeps shadowed duplicate entries.
An unchanged failed module is skipped until you use `--retry-failed`. Changing
the installed module triggers a new scan. Each scan has a ten-second timeout.

Use `--rescan` to inspect unchanged modules again, including failed modules.
`--clear-cache` rebuilds the local catalog; it does not change project data or
uninstall plugins.

`--cached` reads the saved catalog without loading plugin code or checking its
current installation. Its availability labels describe the last scan. JSON
includes the saved module paths, identities, scan status and metadata.

The CLI launches itself in isolated scanner mode; no helper install is required.
Use `--scanner PATH` to select an optional standalone helper. Use `--cache FILE` for a
separate catalog. See the [development build instructions](../development/README.md#plugin-catalog-scanner).

Failed plugin scans produce unavailable entries and do not stop other scans.
Cache/output/scanner setup errors return exit code 1. Invalid arguments return 2.

## Render a project with plugins

```sh
daw-cli render --project MyProject --output mix.wav
daw-cli render --project MyProject --output draft.wav --allow-missing-plugins
```

Rendering uses independent plugin instances, saved opaque state and the shared
engine. It needs no display or audio device. The default is strict: missing or
quarantined plugins fail the export. The explicit degraded option warns for each
missing unit, substitutes a latency-aligned dry effect or silent instrument, and
keeps the saved project unchanged. Available-plugin processing or state failures
still fail. Failed exports do not replace the destination.
