use std::any::Any;
use std::fmt::{format, Debug, Display};
use std::vec::IntoIter;
use colored::{Color, ColoredString, Colorize};

use log::{error, Level, Log, Metadata, Record};

pub struct ConsoleLogger;


impl Log for ConsoleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level().to_level_filter() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) { return }

        let mut level: ColoredString = record.level().as_str().bright_white();
        let mut target: ColoredString = record.target().into();
        let mut args: ColoredString = record.args().to_string().into();

        let mut flavor = [
            ColoredString::from("["),
            ColoredString::from("]"),
            ColoredString::from("  "),
            ColoredString::from(":")];

        match record.level() {
            Level::Error => {
                level.bgcolor  = Some(Color::TrueColor{r:165,g:0,b:0});
                target.fgcolor = Some(Color::TrueColor{r:255,g:55,b:95});
                target.bgcolor = level.bgcolor;
                flavor.iter_mut().for_each(|flavor| {
                    flavor.fgcolor = Some(Color::BrightRed);
                    flavor.bgcolor = level.bgcolor; });
            }
            Level::Warn => {
                flavor[2].input.push_str(" ");
                level.bgcolor  = Some(Color::TrueColor{r:205,g:125,b:0});
                target.fgcolor = Some(Color::TrueColor{r:255,g:255,b:0});
                target.bgcolor = level.bgcolor;
                flavor.iter_mut().for_each(|flavor| {
                    flavor.fgcolor = Some(Color::BrightYellow);
                    flavor.bgcolor = level.bgcolor; });
            }
            Level::Info => {
                flavor[2].input.push_str(" ");
                level.bgcolor  = Some(Color::TrueColor{r:0,g:165,b:0});
                target.fgcolor = Some(Color::TrueColor{r:35,g:255,b:75});
                target.bgcolor = level.bgcolor;
                flavor.iter_mut().for_each(|flavor| {
                    flavor.fgcolor = Some(Color::BrightGreen);
                    flavor.bgcolor = level.bgcolor; });

            }
            Level::Debug => {
                level.bgcolor  = Some(Color::TrueColor{r:0,g:165,b:165});
                target.fgcolor = Some(Color::TrueColor{r:95,g:255,b:235});
                target.bgcolor = level.bgcolor;
                args.fgcolor   = Some(Color::BrightWhite);
                flavor.iter_mut().for_each(|flavor| {
                    flavor.fgcolor = Some(Color::BrightCyan);
                    flavor.bgcolor = level.bgcolor; });
            }
            Level::Trace => {}
        }
        flavor[3].clear_bgcolor();

        eprintln!("{}{}{}{}{}{} {}",
                  flavor[0],level,flavor[1],flavor[2],target,flavor[3],args);
    }

    fn flush(&self) {
    }
}


pub trait Logged<T> {
    fn logged(self, msg: &str) -> T;
}
impl<T,E:Display> Logged<T> for Result<T,E> {
    fn logged(self, msg: &str) -> T {  self.unwrap_or_else(|e| { error!("{}: {}",msg,e); panic!() }) }
}