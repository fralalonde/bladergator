use crate::cache::cache_in;
use crate::core::attributes::{Attributes, Buffered, MetricId, OnFlush, Prefixed, WithAttributes};
use crate::core::error;
use crate::core::input::{Input, InputKind, InputMetric, InputScope};
use crate::core::name::MetricName;
use crate::core::Flush;
use crate::output::format::{Formatting, LineFormat, SimpleFormat};
use crate::queue::queue_in;

use std::sync::Arc;

#[cfg(not(feature = "parking_lot"))]
use std::sync::RwLock;

#[cfg(feature = "parking_lot")]
use parking_lot::RwLock;

use log;
use std::io::Write;
use crate::{Output, OutputScope, OutputMetric};

/// Buffered metrics log output.
#[derive(Clone)]
pub struct Log {
    attributes: Attributes,
    format: Arc<dyn LineFormat>,
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

impl Output for Log {
    type SCOPE = LogScope;

    fn new_scope(&self) -> Self::SCOPE {
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

impl OutputScope for LogScope {
    fn new_metric(&self, name: MetricName, kind: InputKind) -> OutputMetric {
        let name = self.prefix_append(name);
        let template = self.log.format.template(&name, kind);
        let entries = self.entries.clone();

        if self.is_buffered() {
            // buffered
            OutputMetric::new(MetricId::forge("log", name), move |value, labels| {
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
            OutputMetric::new(MetricId::forge("log", name), move |value, labels| {
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

impl InputScope for LogScope {
    fn new_metric(&self, name: MetricName, kind: InputKind) -> InputMetric {
        let name = self.prefix_append(name);
        let template = self.log.format.template(&name, kind);
        let entries = self.entries.clone();

        if self.is_buffered() {
            // buffered
            InputMetric::new(MetricId::forge("log", name), move |value, labels| {
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
            InputMetric::new(MetricId::forge("log", name), move |value, labels| {
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
    use crate::core::input::*;

    #[test]
    fn test_to_log() {
        let c = super::Log::to_log().metrics();
        let m = c.new_metric("test".into(), InputKind::Marker);
        m.write(33, labels![]);
    }

}
