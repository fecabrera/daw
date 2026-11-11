# Symphonia ISO MP4 resource-bound patch

Upstream: symphonia-format-isomp4 0.5.5, MPL-2.0.
Original crates.io archive SHA-256: `243739585d11f81daf8dac8d9f3d18cc7898f6c09a259675fc364b382c30e0a5`.
Original upstream VCS receipt is retained in `.cargo_vcs_info.json`.

DAW changes bound atom/table extents, aggregate parsed metadata and packet storage before allocation; allocation is fallible. Codec algorithms and sample interpretation are unchanged. The host reserves the exported working allowance before probing. Original source headers and license remain intact. This patched source is included in the application's corresponding-source archive.

The patched component uses the application's Rust 1.95 minimum; Cargo.toml.orig retains the original published manifest for provenance.
