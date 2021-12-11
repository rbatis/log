use crate::appender::{FastLogRecord, LogAppender};
use std::cell::RefCell;
use std::fs::{File, OpenOptions, DirBuilder};
use std::io::Write;
use crate::consts::LogSize;
use crate::plugin::file_split::{FileSplitAppender, RollingType};
use crate::plugin::packer::LogPacker;

/// Single logs are stored in rolling mode by capacity
pub struct FileLoopAppender {
    file: FileSplitAppender,
}

impl FileLoopAppender {
    pub fn new(log_file_path: &str, max_temp_size: LogSize) -> FileLoopAppender {
        Self {
            file: FileSplitAppender::new(log_file_path, max_temp_size, RollingType::KeepNum(0), Box::new(LogPacker {}))
        }
    }
}

impl LogAppender for FileLoopAppender {
    fn do_log(&self, record: &mut FastLogRecord) {
        self.file.do_log(record);
    }
}