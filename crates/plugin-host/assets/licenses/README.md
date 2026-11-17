# Plugin host dependency notices

The host uses the workspace-locked CLAP and VST3 binding facades and libloading.
No third-party plugin binary is bundled. Existing package versions are reused;
this change does not upgrade them.

- Retain [CLAP binding notices](../../../plugin-clap/assets/licenses/README.md).
- Retain [VST3 binding notices](../../../plugin-vst3/assets/licenses/README.md),
  including com-scrape-types and the unresolved generated-binding provenance
  checks described in the hosting specification.
- libloading 0.8.9: ISC; retain `Libloading-LICENSE.txt` from the published Cargo
  package. Source: https://github.com/nagisa/rust_libloading/tree/0.8.9.

Portable model, serde, hashing, error and platform dependency notices remain
required. This directory is not a complete distribution clearance. Scanner/CLI
and desktop helper packages must retain their full dependency notices.
