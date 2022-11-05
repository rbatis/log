use log::{LevelFilter, Log, Metadata, Record};
use std::ops::Deref;
use std::sync::atomic::{AtomicI64, Ordering};

use crate::appender::{Command, FastLogRecord};
use crate::config::Config;
use crate::error::LogError;
use crate::filter::Filter;
use crate::{chan, spawn, Receiver, SendError, Sender, WaitGroup};
use once_cell::sync::{Lazy, OnceCell};
use std::result::Result::Ok;
use std::sync::Arc;
use std::time::SystemTime;

pub struct Chan {
    pub filter: OnceCell<Box<dyn Filter>>,
    pub send: Sender<FastLogRecord>,
    pub recv: Receiver<FastLogRecord>,
}

impl Chan {
    pub fn new(len: Option<usize>) -> Self {
        let (s, r) = chan(len);
        Chan {
            filter: OnceCell::new(),
            send: s,
            recv: r,
        }
    }
    pub fn set_filter(&self, f: Box<dyn Filter>) {
        self.filter.get_or_init(|| f);
        self.filter.get();
    }
}

pub struct Logger {
    pub chan: Chan,
}

impl Logger {
    pub fn set_level(&self, level: LevelFilter) {
        log::set_max_level(level);
    }

    pub fn get_level(&self) -> LevelFilter {
        log::max_level()
    }

    /// print no other info
    pub fn print(&self, log: String) -> Result<(), SendError<FastLogRecord>> {
        let fast_log_record = FastLogRecord {
            command: Command::CommandRecord,
            level: log::Level::Info,
            target: "".to_string(),
            args: "".to_string(),
            module_path: "".to_string(),
            file: "".to_string(),
            line: None,
            now: SystemTime::now(),
            formated: log,
        };
        LOGGER.chan.send.send(fast_log_record)
    }

    pub fn wait(&self) {
        self.flush();
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.get_level()
    }
    fn log(&self, record: &Record) {
        if let Some(filter) = LOGGER.chan.filter.get() {
            if !filter.filter(record) {
                let fast_log_record = FastLogRecord {
                    command: Command::CommandRecord,
                    level: record.level(),
                    target: record.metadata().target().to_string(),
                    args: record.args().to_string(),
                    module_path: record.module_path().unwrap_or_default().to_string(),
                    file: record.file().unwrap_or_default().to_string(),
                    line: record.line().clone(),
                    now: SystemTime::now(),
                    formated: String::new(),
                };
                LOGGER.chan.send.send(fast_log_record);
            }
        }
    }
    fn flush(&self) {
        match flush() {
            Ok(v) => {
                v.wait();
            }
            Err(_) => {}
        }
    }
}

static CHAN_LEN: AtomicI64 = AtomicI64::new(-1);
pub static LOGGER: Lazy<Logger> = Lazy::new(|| Logger {
    chan: Chan::new({
        let len = CHAN_LEN.load(Ordering::SeqCst);
        match len {
            -1 => None,
            v => Some(v as usize),
        }
    }),
});

pub fn init(config: Config) -> Result<&'static Logger, LogError> {
    if config.appends.is_empty() {
        return Err(LogError::from("[fast_log] appenders can not be empty!"));
    }
    match config.chan_len {
        None => {
            CHAN_LEN.store(-1, Ordering::SeqCst);
        }
        Some(v) => {
            CHAN_LEN.store(v as i64, Ordering::SeqCst);
        }
    }
    LOGGER.set_level(config.level);
    LOGGER.chan.set_filter(config.filter);
    //main recv data
    let appenders = config.appends;
    let format = Arc::new(config.format);
    let level = config.level;
    let chan_len = config.chan_len;
    log::set_logger(LOGGER.deref())
        .map(|()| log::set_max_level(level))
        .map_err(|e| LogError::from(e))?;
    std::thread::spawn(move || {
        let mut recever_vec = vec![];
        let mut sender_vec: Vec<Sender<Arc<Vec<FastLogRecord>>>> = vec![];
        for a in appenders {
            let (s, r) = chan(chan_len);
            sender_vec.push(s);
            recever_vec.push((r, a));
        }
        for (recever, appender) in recever_vec {
            spawn(move || {
                let mut exit = false;
                loop {
                    //batch fetch
                    if let Ok(msg) = recever.recv() {
                        appender.do_logs(msg.as_ref());
                        for x in msg.iter() {
                            match x.command {
                                Command::CommandRecord => {}
                                Command::CommandExit => {
                                    exit = true;
                                    continue;
                                }
                                Command::CommandFlush(_) => {
                                    appender.flush();
                                    continue;
                                }
                            }
                        }
                    }
                    if exit {
                        break;
                    }
                }
            });
        }
        loop {
            let mut remain = Vec::with_capacity(1000);
            //recv
            if LOGGER.chan.recv.len() == 0 {
                if let Ok(item) = LOGGER.chan.recv.recv() {
                    remain.push(item);
                }
            } else {
                recv_all(&mut remain, &LOGGER.chan.recv);
            }
            let mut exit = false;
            for x in &mut remain {
                if x.formated.is_empty() {
                    format.do_format(x);
                }
                if x.command.eq(&Command::CommandExit) {
                    exit = true;
                }
            }
            let data = Arc::new(remain);
            for x in &sender_vec {
                x.send(data.clone());
            }
            if exit {
                break;
            }
        }
    });
    return Ok(LOGGER.deref());
}

fn recv_all<T>(data: &mut Vec<T>, recver: &Receiver<T>) {
    loop {
        match recver.try_recv() {
            Ok(v) => {
                data.push(v);
            }
            Err(_) => {
                break;
            }
        }
    }
}

pub fn exit() -> Result<(), LogError> {
    let fast_log_record = FastLogRecord {
        command: Command::CommandExit,
        level: log::Level::Info,
        target: String::new(),
        args: String::new(),
        module_path: String::new(),
        file: String::new(),
        line: None,
        now: SystemTime::now(),
        formated: String::new(),
    };
    let result = LOGGER.chan.send.send(fast_log_record);
    match result {
        Ok(()) => {
            return Ok(());
        }
        _ => {}
    }
    return Err(LogError::E("[fast_log] exit fail!".to_string()));
}

pub fn flush() -> Result<WaitGroup, LogError> {
    let wg = WaitGroup::new();
    let fast_log_record = FastLogRecord {
        command: Command::CommandFlush(wg.clone()),
        level: log::Level::Info,
        target: String::new(),
        args: String::new(),
        module_path: String::new(),
        file: String::new(),
        line: None,
        now: SystemTime::now(),
        formated: String::new(),
    };
    let result = LOGGER.chan.send.send(fast_log_record);
    match result {
        Ok(()) => {
            return Ok(wg);
        }
        _ => {}
    }
    return Err(LogError::E("[fast_log] flush fail!".to_string()));
}

pub fn print(log: String) -> Result<(), SendError<FastLogRecord>> {
    LOGGER.print(log)
}
