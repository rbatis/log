use log::{LevelFilter};
use crate::appender::{FastLogFormatRecord, LogAppender, RecordFormat};
use crate::consts::LogSize;
use crate::filter::{Filter, NoFilter};
use crate::plugin::console::ConsoleAppender;
use crate::plugin::file::FileAppender;
use crate::plugin::file_loop::FileLoopAppender;
use crate::plugin::file_split::{FileSplitAppender, Packer, RollingType};

pub struct Config {
    pub appenders: Vec<Box<dyn LogAppender>>,
    pub level: LevelFilter,
    pub filter: Box<dyn Filter>,
    pub format: Box<dyn RecordFormat>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            appenders: vec![],
            level: LevelFilter::Info,
            filter: Box::new(NoFilter {}),
            format: Box::new(FastLogFormatRecord::new()),
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    /// set log LevelFilter
    pub fn level(mut self, level: LevelFilter) -> Self {
        self.level = level;
        self
    }
    /// set log Filter
    pub fn filter<F: Filter + 'static>(mut self, filter: F) -> Self {
        self.filter = Box::new(filter);
        self
    }
    /// set log format
    pub fn format<F: RecordFormat + 'static>(mut self, format: F) -> Self {
        self.format = Box::new(format);
        self
    }
    /// add a ConsoleAppender
    pub fn console(mut self) -> Self {
        self.appenders.push(Box::new(ConsoleAppender {}));
        self
    }
    /// add a FileAppender
    pub fn file(mut self, file: &str) -> Self {
        self.appenders.push(Box::new(FileAppender::new(file)));
        self
    }
    /// add a FileLoopAppender
    pub fn file_loop(mut self, file: &str, max_temp_size: LogSize) -> Self {
        self.appenders.push(Box::new(FileLoopAppender::new(file, max_temp_size)));
        self
    }
    /// add a FileSplitAppender
    pub fn file_split<P: Packer + 'static>(mut self, file_path: &str,
                                           max_temp_size: LogSize,
                                           rolling_type: RollingType,
                                           packer: P, ) -> Self {
        self.appenders.push(Box::new(FileSplitAppender::new(file_path, max_temp_size, rolling_type, Box::new(packer))));
        self
    }
    /// add a custom LogAppender
    pub fn custom<Appender: LogAppender + 'static>(mut self, arg: Appender) -> Self {
        self.appenders.push(Box::new(arg));
        self
    }
}