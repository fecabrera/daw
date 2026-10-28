# Validate a project

```sh
daw-cli validate --project examples/demo
```

`--project` is required and points to a folder containing `project.json`. Validation checks the saved project and opens referenced audio. It uses the same project loading and preparation as export.

A valid project returns exit code 0 and reports its name and track count. Missing or shortened sources produce warnings; missing audio renders as silence. Invalid project data, unsupported or damaged audio, and preparation failures return exit code 1. Invalid command arguments return exit code 2.

Validation does not save changes. Use [tracks](tracks.md) for metadata inspection without loading audio.

[CLI commands](README.md)
