use cache::cache_in;
use core::attributes::{Attributes, Buffered, OnFlush, Prefixed, WithAttributes};
use core::error;
use core::input::{Input, InputKind, InputMetric, InputScope};
use core::name::MetricName;
use core::Flush;
use cache::gapless_in;
use output::format::{Formatting, LineFormat, SimpleFormat};
use queue::queue_in;

use std::sync::Arc;

#[cfg(not(feature = "parking_lot"))]
use std::sync::RwLock;

#[cfg(feature = "parking_lot")]
use parking_lot::RwLock;

use log;
use std::io::Write;

/// Buffered metrics log output.
#[derive(Clone)]
pub struct Log {
    attributes: Attributes,
    format: Arc<LineFormat>,
    level: log::Level,
    target: Option<String>,
}

impl Input for Log {
    type SCOPE = LogScope;

    fn metrics(&self) -> Self::SCOPE {
        LogScope {
            attributes: self.attributes.clone(),
            entries: Arc::new(RwLock::new(Vec::new())),
            log: self.clone(),
        }
    }
}

impl WithAttributes for Log {
    fn get_attributes(&self) -> &Attributes {
        &self.attributes
    }
    fn mut_attributes(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
}

impl Buffered for Log {}

impl Formatting for Log {
    fn formatting(&self, format: impl LineFormat + 'static) -> Self {
        let mut cloned = self.clone();
        cloned.format = Arc::new(format);
        cloned
    }
}

/// A scope for metrics log output.
#[derive(Clone)]
pub struct LogScope {
    attributes: Attributes,
    entries: Arc<RwLock<Vec<Vec<u8>>>>,
    log: Log,
}

impl Log {
    /// Write metric values to the standard log using `info!`.
    // TODO parameterize log level, logger
    pub fn to_log() -> Log {
        Log {
            attributes: Attributes::default(),
            format: Arc::new(SimpleFormat::default()),
            level: log::Level::Info,
            target: None,
        }
    }

    /// Sets the log `target` to use when logging metrics.
    /// See the (log!)[https://docs.rs/log/0.4.6/log/macro.log.html] documentation.
    pub fn level(&self, level: log::Level) -> Self {
        let mut cloned = self.clone();
        cloned.level = level;
        cloned
    }

    /// Sets the log `target` to use when logging metrics.
    /// See the (log!)[https://docs.rs/log/0.4.6/log/macro.log.html] documentation.
    pub fn target(&self, target: &str) -> Self {
        let mut cloned = self.clone();
        cloned.target = Some(target.to_string());
        cloned
    }
}

impl WithAttributes for LogScope {
    fn get_attributes(&self) -> &Attributes {
        &self.attributes
    }
    fn mut_attributes(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
}

impl Buffered for LogScope {}

impl queue_in::QueuedInput for Log {}
impl cache_in::CachedInput for Log {}

impl gapless_in::Gapless for Log {}

impl InputScope for LogScope {
    fn new_metric(&self, name: MetricName, kind: InputKind) -> InputMetric {
        let name = self.prefix_append(name);
        let template = self.log.format.template(&name, kind);
        let entries = self.entries.clone();

        if self.is_buffered() {
            // buffered
            InputMetric::new(move |value, labels| {
                let mut buffer = Vec::with_capacity(32);
                match template.print(&mut buffer, value, |key| labels.lookup(key)) {
                    Ok(()) => {
                        let mut entries = write_lock!(entries);
                        entries.push(buffer)
                    }
                    Err(err) => debug!("Could not format buffered log metric: {}", err),
                }
            })
        } else {
            // unbuffered
            let level = self.log.level;
            let target = self.log.target.clone();
            InputMetric::new(move |value, labels| {
                let mut buffer = Vec::with_capacity(32);
                match template.print(&mut buffer, value, |key| labels.lookup(key)) {
                    Ok(()) => {
                        if let Some(target) = &target {
                            log!(target: target, level, "{:?}", &buffer)
                        } else {
                            log!(level, "{:?}", &buffer)
                        }
                    }
                    Err(err) => debug!("Could not format buffered log metric: {}", err),
                }
            })
        }
    }
}

impl Flush for LogScope {
    fn flush(&self) -> error::Result<()> {
        self.notify_flush_listeners();
        let mut entries = write_lock!(self.entries);
        if !entries.is_empty() {
            let mut buf: Vec<u8> = Vec::with_capacity(32 * entries.len());
            for entry in entries.drain(..) {
                writeln!(&mut buf, "{:?}", &entry)?;
            }
            if let Some(target) = &self.log.target {
                log!(target: target, self.log.level, "{:?}", &buf)
            } else {
                log!(self.log.level, "{:?}", &buf)
            }
        }
        Ok(())
    }
}

impl Drop for LogScope {
    fn drop(&mut self) {
        if let Err(e) = self.flush() {
            warn!("Could not flush log metrics on Drop. {}", e)
        }
    }
}

#[cfg(test)]
mod test {
    use core::input::*;

    #[test]
    fn test_to_log() {
        let c = super::Log::to_log().metrics();
        let m = c.new_metric("test".into(), InputKind::Marker);
        m.write(33, labels![]);
    }

}
