# Collect project sources

Copy original media into a saved project before moving or sharing its folder. Save and close the desktop project first. The command uses the saved project and runs without a confirmation prompt.

```sh
daw-cli collect-sources [--project <folder>] [--dry-run]
```

`--project` defaults to the current directory. Quote paths that contain spaces.

```sh
daw-cli collect-sources --project "My project" --dry-run
daw-cli collect-sources --project "My project"
```

`--dry-run` checks sources and reports files to copy, files already included and additional bytes. It does not copy files or save changes. Allow extra space for temporary files; the command does not measure available disk space. A successful check does not guarantee that a later collection will succeed.

Collection copies complete original media without trimming or re-encoding. It includes unused saved assets and Sampler/Granulizer inputs. Originals stay unchanged. Embedded MIDI stays in the project. Desktop Undo/Redo history is unavailable to this command; use desktop collection before closing the app if those sources must travel too.

After success, move the whole project folder, including `assets/` and `stems/`. Later imports stay external until another collection. Running collection again copies only media that still needs inclusion and saves the project even when no new files are needed.

Results appear on standard output. Warnings and source problems appear on standard error with affected paths and assets. Exit codes are:

| Code | Meaning |
| --- | --- |
| 0 | Collection was confirmed and saved, or a dry run found no blocking problems. |
| 1 | Project, media, write or verification failure; collection blocked; or saving could not be confirmed. |
| 2 | Invalid command arguments. |

A missing required source blocks collection, including `--dry-run`. Repair the source and retry. This differs from validation and rendering, which can use silence for missing sources.

A failure before saving retains the previous `project.json`. If the command cannot confirm that saving completed, keep all project media and the candidate project file named in the error for recovery. Recovery is not automatic. Interrupting the process can leave unreferenced files; the command does not remove or reuse crash leftovers. Do not change the project from another process while collection runs.

See [project source collection](../projects/README.md#collect-project-sources) for the desktop workflow.
