# Charon provider-policy compatibility patch

Source: <https://github.com/Valkyra-Labs/charon-audio/tree/bc045cd80f009ea863772c5afef15928101df66f> (0.1.2, MIT OR Apache-2.0).

This directory retains the upstream manifest, library/binary source, README, licenses, tests and benchmark. It contains no pretrained model weights. The workspace excludes this dependency from its own member list and overrides the pinned Git dependency through Cargo's source patch mechanism.

The only processing-source change is in `src/models.rs::OnnxModel::build_session`: set the automatic device policy to `Default` before registering the explicit CPU/CoreML providers. ort 2.0.0-rc.13 starts session builders with `MaxEfficiency`. Its CPU provider registration only configures the arena, leaving that policy active. With native runtime 1.31 this can select CoreML for an in-graph CPU model, causing model compilation or shape errors. Resetting the policy preserves the selected provider and the existing low-memory preset. No inference, windowing, transform, blending, or output code is changed.

Keep this source patch until an upstream Charon/ort revision provides equivalent explicit-provider behavior without breaking the current API. Validate removal with CPU, CoreML and failed-CoreML-to-CPU smoke tests, and the malformed-model regression in `daw-separation`. Keep the upstream license texts in this directory and the packaged separation notices.
