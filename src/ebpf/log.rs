use log::{Level, LevelFilter, Log, Metadata, Record};

struct PrintlnLogger;

impl Log for PrintlnLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let line = format!("[{} {}] {}", record.level(), record.target(), record.args());
        if record.level() <= Level::Warn {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    }

    fn flush(&self) {}
}

static LOGGER: PrintlnLogger = PrintlnLogger;

pub fn init() {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(LevelFilter::Trace);
}
