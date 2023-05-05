#![allow(clippy::missing_safety_doc)]

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
compile_error!("Unsupported OS");
