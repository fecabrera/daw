//! Stable presentation for dotted commands. Legacy commands keep their own stdout.
use serde_json::{Value, json};
use std::io::{self, Write};

pub(super) struct Outcome {
    text: String,
    result: Value,
    committed: bool,
}

impl Outcome {
    pub(super) fn read(text: String, result: Value) -> Self {
        Self {
            text,
            result,
            committed: false,
        }
    }

    pub(super) fn committed(text: String, result: Value) -> Self {
        Self {
            text,
            result,
            committed: true,
        }
    }

    pub(super) fn write(self, command: &str, json_output: bool) -> daw_project::Result<()> {
        let output = if json_output {
            format!(
                "{}\n",
                json!({"schema_version": 1, "command": command, "result": self.result})
            )
        } else {
            self.text
        };
        if let Err(error) = io::stdout().lock().write_all(output.as_bytes())
            && error.kind() != io::ErrorKind::BrokenPipe
        {
            if self.committed {
                warning(&format!(
                    "Operation completed, but result output failed: {error}"
                ));
            } else {
                return Err(daw_project::Error(format!("Cannot write result: {error}")));
            }
        }
        Ok(())
    }
}

pub(super) fn escaped(message: &str) -> String {
    let mut text = String::with_capacity(message.len());
    for ch in message.chars() {
        if ch.is_control() {
            text.extend(ch.escape_default());
        } else {
            text.push(ch);
        }
    }
    text
}

pub(super) fn warning(message: &str) {
    let _ = writeln!(io::stderr().lock(), "Warning: {}", escaped(message));
}
