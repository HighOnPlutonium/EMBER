use colored::{Color, ColoredString, Colorize};
use log::{Level, Log, Metadata, Record};

pub struct ConsoleLogger;

impl Log for ConsoleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level().to_level_filter() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) { return }

        let mut level: ColoredString = record.level().as_str().into();
        let mut target: ColoredString = record.target().into();
        let mut args: ColoredString = record.args().to_string().into();

        match record.level() {
            Level::Error => {
                level.bgcolor = Some(Color::BrightRed);
                target.fgcolor = level.bgcolor;
                target.bgcolor = level.bgcolor;
            }
            Level::Warn => {
                level.bgcolor = Some(Color::BrightYellow);
                target.bgcolor = level.bgcolor;
                target.fgcolor = target.bgcolor;
            }
            Level::Info => {}
            Level::Debug => {
                level.bgcolor = Some(Color::BrightCyan);
                target.fgcolor = level.bgcolor;
                target.bgcolor = level.bgcolor;
            }
            Level::Trace => {}
        }
        let mut flavor = [
            ColoredString::from("["),
            ColoredString::from("]"),
            ColoredString::from("  "),
            ColoredString::from(":")
        ];
        flavor.iter_mut().for_each(|flavor|{ flavor.fgcolor = level.bgcolor; flavor.bgcolor = level.bgcolor; });

        eprintln!("{}{}{}{}{}{} {}",
                  flavor[0],level,flavor[1],flavor[2],target,flavor[3],args);
    }

    fn flush(&self) {
    }
}

