use std::fmt;
use std::fmt::Formatter;
use colored::Colorize;
use log::error;

pub(crate) mod linux;
pub(crate) mod windows;


#[cfg(target_os = "linux")]
const PLATFORM: Platform = Platform::LINUX;
#[cfg(windows)]
const PLATFORM: Platform = Platform::WINDOWS;

enum Platform {
    LINUX,
    WINDOWS,
}
impl fmt::Display for Platform {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::LINUX => f.write_str("Linux"),
            Self::WINDOWS => f.write_str("Windows"),
        }
    }
}

pub fn platform_mismatch(platform: Platform) -> ! {
    error!("{}-specific functionality missing on {}-build!",
        platform.to_string().bright_purple(),
        PLATFORM.to_string().bright_purple());
    panic!()
}