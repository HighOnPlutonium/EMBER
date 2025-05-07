use std::any::Any;
use std::ffi;
use std::ffi::CStr;
use std::fmt::{format, Debug, Display};
use std::vec::IntoIter;
use ash::{vk, Instance};
use colored::{Color, ColoredString, Colorize};

use log::{debug, error, info, trace, warn, Level, Log, Metadata, Record};

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
        if target.input == "VULKAN".to_owned() { flavor[2].input = " ".to_owned() }
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
            Level::Trace => {
                level.bgcolor = Some(Color::TrueColor{r:65,g:65,b:65});
                level.fgcolor = Some(Color::BrightWhite);
                target.bgcolor = level.bgcolor;
                target.fgcolor = Some(Color::BrightWhite);
                flavor.iter_mut().for_each(|flavor| {
                    flavor.fgcolor = Some(Color::TrueColor{r:205,g:205,b:205});
                    flavor.bgcolor = level.bgcolor;
                });

            }
        }
        flavor[3].clear_bgcolor();

        eprintln!("{}{}{}{}{}{} {}",
                  flavor[0],level,flavor[1],flavor[2],target,flavor[3],args);
    }

    fn flush(&self) {
    }
}
#[allow(unused)]
pub(crate) unsafe extern "system" fn  debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    msg_type: vk::DebugUtilsMessageTypeFlagsEXT,
    callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    user_data: *mut ffi::c_void)
    -> vk::Bool32 {
    let message = CStr::from_ptr((*callback_data).p_message).to_str().unwrap();

    {   type Flags = vk::DebugUtilsMessageSeverityFlagsEXT;
        match severity {
            Flags::INFO => { info!("info") }
            Flags::WARNING => { warn!("warning") }
            Flags::ERROR => { error!("error") }
            Flags::VERBOSE => { trace!(target:"VULKAN","{:?}: {}",msg_type,message) }
            _ => { trace!("?") }
    }}

    false.into()
}

#[allow(unused)]
pub(crate) unsafe extern "system" fn debug_reporter(
    flags: vk::DebugReportFlagsEXT,
    object_type: vk::DebugReportObjectTypeEXT,
    object: u64,
    location: usize,
    message_code: i32,
    layer_prefix: *const ffi::c_char,
    message: *const ffi::c_char,
    user_data: *mut ffi::c_void,
) -> vk::Bool32 {

    let message = CStr::from_ptr(message);

    { type Flags = vk::DebugReportFlagsEXT;
        match flags {
            Flags::PERFORMANCE_WARNING => { warn!(target:"VULKAN","{:?}",message) }
            Flags::WARNING => { warn!(target:"VULKAN","{:?}",message) }
            Flags::DEBUG => { debug!(target:"VULKAN","{:?}",message) }
            Flags::ERROR => { error!(target:"VULKAN","{:?}",message) }
            Flags::INFORMATION => { info!(target:"VULKAN","{:?}",message) }
            _ => { trace!(target:"VULKAN","??? {:?}",message) }
        }

    }

    false.into()
}




pub trait Logged<T> {
    fn logged(self, msg: &str) -> T;
}
impl<T,E:Display> Logged<T> for Result<T,E> {
    // todo!("add cleanup calls")
    fn logged(self, msg: &str) -> T {  self.unwrap_or_else(|e| { error!("{}: {}",msg,e); panic!() }) }
}