use std::error::Error;
use std::process::Command;
use log::{error, info, warn};

pub fn get_pid(name: &str, all_users: bool) -> Result<usize, Box<dyn Error>> {
    let name = name.trim();
    Ok(String::from_utf8_lossy(
        &Command::new("ps")
            .arg(if all_users { "ax" } else { "x" })
            .output()?
            .stdout.as_slice())
        .lines()
        .filter(|x| x.contains(format!("/{} ", name).as_str()))
        .next()
        .ok_or_else(||{ error!("No Process named \"{}\" found", name); panic!() }).unwrap()
        .trim()
        .split_once(' ')
        .unwrap()
        .0.parse::<usize>()?)
}


pub struct Root(u32);
impl Root {
    pub fn new() -> Self {
        unsafe { Root(libc::getuid() ) }
    }

    pub unsafe fn claim(&mut self) {
        if libc::getuid() == 0 {
            warn!("Unnecessary root claim");
        }
        if libc::setuid(0) != 0 {
            error!("Couldn't claim root");
            panic!();
        }
        warn!("Claimed root");
    }

    pub unsafe fn release(&mut self) {
        let uid = libc::getuid();
        if uid == 0 {
            libc::setuid(self.0);
            info!("Released root");
        } else if uid == self.0 {
            warn!("Unnecessary root release");
        } else {
            error!("Current non-root UID does not match stored UID!");
            panic!();
        }
    }
}


pub fn elf_offset(path: &str, symbol: &str) -> usize {
    usize::from_str_radix(
        &String::from_utf8(
            Command::new("readelf")
                .args(["-WCs", path])
                .output()
                .expect("readelf execute failed")
                .stdout,
        )
            .expect("failed to parse readelf")
            .split_once(symbol)
            .expect(&format!("cannot find symbol \"{}\"",symbol))
            .0
            .rsplit_once('\n')
            .expect(&format!("cannot read offset of \"{}\"",symbol))
            .1
            .split_once(':')
            .expect("parse `:` failed.")
            .1
            .trim()
            .split_once(' ')
            .expect("cannot parse space")
            .0,
        16,
    )
        .expect("failed to process readelf.")
}