//! Header checks reject foreign architectures before invoking a native loader.
use crate::{Error, Result};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub(crate) fn check_native_binary(path: &Path) -> Result<()> {
    let mut file = File::open(path)?;
    let mut header = [0; 64];
    file.read_exact(&mut header)?;
    let compatible = if cfg!(target_os = "macos") {
        let cpu = if cfg!(target_arch = "aarch64") {
            0x0100_000c
        } else {
            0x0100_0007
        };
        match &header[..4] {
            [0xcf, 0xfa, 0xed, 0xfe] => u32::from_le_bytes(header[4..8].try_into().unwrap()) == cpu,
            [0xca, 0xfe, 0xba, 0xbe] | [0xca, 0xfe, 0xba, 0xbf] => {
                let wide = header[3] == 0xbf;
                let count = u32::from_be_bytes(header[4..8].try_into().unwrap());
                if count > 32 {
                    return Err(Error::Invalid(
                        "Mach-O bundle has more than 32 architecture slices".into(),
                    ));
                }
                let mut found = false;
                file.seek(SeekFrom::Start(8))?;
                for _ in 0..count {
                    let mut entry = [0; 32];
                    file.read_exact(&mut entry[..if wide { 32 } else { 20 }])?;
                    found |= u32::from_be_bytes(entry[..4].try_into().unwrap()) == cpu;
                }
                found
            }
            _ => false,
        }
    } else if cfg!(target_os = "windows") {
        if &header[..2] != b"MZ" {
            false
        } else {
            let offset = u32::from_le_bytes(header[60..64].try_into().unwrap());
            file.seek(SeekFrom::Start(u64::from(offset)))?;
            let mut pe = [0; 6];
            file.read_exact(&mut pe)?;
            let machine = if cfg!(target_arch = "aarch64") {
                0xaa64
            } else if cfg!(target_arch = "x86_64") {
                0x8664
            } else {
                0x014c
            };
            &pe[..4] == b"PE\0\0" && u16::from_le_bytes([pe[4], pe[5]]) == machine
        }
    } else {
        let machine = if cfg!(target_arch = "aarch64") {
            183
        } else if cfg!(target_arch = "x86_64") {
            62
        } else {
            3
        };
        &header[..4] == b"\x7fELF"
            && header[4]
                == if cfg!(target_pointer_width = "64") {
                    2
                } else {
                    1
                }
            && header[5] == 1
            && u16::from_le_bytes([header[18], header[19]]) == machine
    };
    if compatible {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "Plugin is not a native {} {} executable: {}",
            std::env::consts::OS,
            std::env::consts::ARCH,
            path.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_test_binary_passes_and_text_does_not() {
        check_native_binary(&std::env::current_exe().unwrap()).unwrap();
        assert!(
            check_native_binary(Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/Cargo.toml"
            )))
            .is_err()
        );
    }
}
