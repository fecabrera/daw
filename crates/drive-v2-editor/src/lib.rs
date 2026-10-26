//! Drive v2 editor using the shared bounded control model.
use daw_unit_ui::drive_v2 as body;
mod editor {
    include!("editor.rs");
    #[cfg(test)]
    mod tests {
        include!("tests.rs");
    }
}
pub use editor::*;
