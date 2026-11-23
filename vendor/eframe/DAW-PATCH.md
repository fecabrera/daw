# DAW secondary viewport patch

Source is eframe 0.36.2 from crates.io, at egui revision
49682f8baa058bf49e011035cfbd6e825f88a5ef. This copy uses the upstream MIT grant.
The DAW adds opt-in status, child-local startup and swap error handling, and
root-only automatic geometry persistence. Presented watched viewports also
reuse their submitted frame on move events. Resize and scale changes retain
the normal redraw path. Read the upstream README for the original crate.
