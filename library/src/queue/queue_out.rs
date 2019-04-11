//! Queue metrics for write on a separate thread,
//! RawMetrics definitions are still synchronous.
//! If queue size is exceeded, calling code reverts to blocking.

use cache::cache_in;
use core::attributes::{Attributes, OnFlush, Prefixed, WithAttributes};
use core::error;
use core::input::{Input, InputKind, InputMetric, InputScope};
use core::label::Labels;
use core::metrics;
use core::name::MetricName;
use core::output::{Output, OutputDyn, OutputMetric, OutputScope};
use core::{Flush, MetricValue};

use std::fmt;
use std::ops;
use std::rc::Rc;

#[cfg(not(feature = "crossbeam-channel"))]
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

#[cfg(feature = "crossbeam-channel")]
use crossbeam_channel as crossbeam;

/// Wrap this raw output behind an asynchronous metrics dispatch queue.
pub trait QueuedOutput: Output + Sized {
    /// Wrap this output with an asynchronous dispatch queue.
    fn queued(self, max_size: usize) -> OutputQueue {
        OutputQueue::new(self, max_size)
    }
}

/// # Panics
///
/// Panics if the OS fails to create a thread.
#[cfg(not(feature = "crossbeam-channel"))]
fn new_async_channel(length: usize) -> Arc<mpsc::SyncSender<OutputQueueCmd>> {
    let (sender, receiver) = mpsc::sync_channel::<OutputQueueCmd>(length);

    thread::Builder::new()
        .name("dipstick-queue-out".to_string())
        .spawn(move || {
            let mut done = false;
            while !done {
                match receiver.recv() {
                    Ok(OutputQueueCmd::Write(metric, value, labels)) => metric.write(value, labels),
                    Ok(OutputQueueCmd::Flush(scope)) => {
                        if let Err(e) = scope.flush() {
                            debug!("Could not asynchronously flush metrics: {}", e);
                        }
                    }
                    Err(e) => {
                        debug!("Async metrics receive loop terminated: {}", e);
                        // cannot break from within match, use safety pin instead
                        done = true
                    }
                }
            }
        })
        .unwrap(); // TODO: Panic, change API to return Result?
    Arc::new(sender)
}

/// # Panics
///
/// Panics if the OS fails to create a thread.
#[cfg(feature = "crossbeam-channel")]
fn new_async_channel(length: usize) -> Arc<crossbeam::Sender<OutputQueueCmd>> {
    let (sender, receiver) = crossbeam::bounded::<OutputQueueCmd>(length);

    thread::Builder::new()
        .name("dipstick-queue-out".to_string())
        .spawn(move || {
            let mut done = false;
            while !done {
                match receiver.recv() {
                    Ok(OutputQueueCmd::Write(metric, value, labels)) => metric.write(value, labels),
                    Ok(OutputQueueCmd::Flush(scope)) => {
                        if let Err(e) = scope.flush() {
                            debug!("Could not asynchronously flush metrics: {}", e);
                        }
                    }
                    Err(e) => {
                        debug!("Async metrics receive loop terminated: {}", e);
                        // cannot break from within match, use safety pin instead
                        done = true
                    }
                }
            }
        })
        .unwrap(); // TODO: Panic, change API to return Result?
    Arc::new(sender)
}

/// Wrap scope with an asynchronous metric write & flush dispatcher.
#[derive(Clone)]
pub struct OutputQueue {
    attributes: Attributes,
    target: Arc<OutputDyn + Send + Sync + 'static>,
    #[cfg(not(feature = "crossbeam-channel"))]
    q_sender: Arc<mpsc::SyncSender<OutputQueueCmd>>,
    #[cfg(feature = "crossbeam-channel")]
    q_sender: Arc<crossbeam::Sender<OutputQueueCmd>>,
}

impl OutputQueue {
    /// Wrap new scopes with an asynchronous metric write & flush dispatcher.
    pub fn new<OUT: Output + Send + Sync + 'static>(target: OUT, queue_length: usize) -> Self {
        OutputQueue {
            attributes: Attributes::default(),
            target: Arc::new(target),
            q_sender: new_async_channel(queue_length),
        }
    }
}

impl WithAttributes for OutputQueue {
    fn get_attributes(&self) -> &Attributes {
        &self.attributes
    }
    fn mut_attributes(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
}

impl cache_in::CachedInput for OutputQueue {}

impl Input for OutputQueue {
    type SCOPE = OutputQueueScope;

    /// Wrap new scopes with an asynchronous metric write & flush dispatcher.
    fn metrics(&self) -> Self::SCOPE {
        let target_scope = UnsafeScope::new(self.target.output_dyn());
        OutputQueueScope {
            attributes: self.attributes.clone(),
            sender: self.q_sender.clone(),
            target: Arc::new(target_scope),
        }
    }
}

/// This is only `pub` because `error` module needs to know about it.
/// Async commands should be of no concerns to applications.
pub enum OutputQueueCmd {
    /// Send metric write
    Write(Arc<OutputMetric>, MetricValue, Labels),
    /// Send metric flush
    Flush(Arc<UnsafeScope>),
}

/// A scope wrapper that sends writes & flushes over a Rust sync channel.
/// Commands are executed by a background thread.
#[derive(Clone)]
pub struct OutputQueueScope {
    attributes: Attributes,
    #[cfg(not(feature = "crossbeam-channel"))]
    sender: Arc<mpsc::SyncSender<OutputQueueCmd>>,
    #[cfg(feature = "crossbeam-channel")]
    sender: Arc<crossbeam::Sender<OutputQueueCmd>>,
    target: Arc<UnsafeScope>,
}

impl WithAttributes for OutputQueueScope {
    fn get_attributes(&self) -> &Attributes {
        &self.attributes
    }
    fn mut_attributes(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
}

impl InputScope for OutputQueueScope {
    fn new_metric(&self, name: MetricName, kind: InputKind) -> InputMetric {
        let name = self.prefix_append(name);
        let target_metric = Arc::new(self.target.new_metric(name, kind));
        let sender = self.sender.clone();
        InputMetric::new(move |value, mut labels| {
            labels.save_context();
            if let Err(e) = sender.send(OutputQueueCmd::Write(target_metric.clone(), value, labels))
            {
                metrics::SEND_FAILED.mark();
                debug!("Failed to send async metrics: {}", e);
            }
        })
    }
}

impl Flush for OutputQueueScope {
    fn flush(&self) -> error::Result<()> {
        self.notify_flush_listeners();
        if let Err(e) = self.sender.send(OutputQueueCmd::Flush(self.target.clone())) {
            metrics::SEND_FAILED.mark();
            debug!("Failed to flush async metrics: {}", e);
            Err(e.into())
        } else {
            Ok(())
        }
    }
}

/// Wrap an OutputScope to make it Send + Sync, allowing it to travel the world of threads.
/// Obviously, it should only still be used from a single thread or dragons may occur.
#[derive(Clone)]
pub struct UnsafeScope(Rc<OutputScope + 'static>);

/// This is ok because scope will only ever be used by the dispatcher thread.
unsafe impl Send for UnsafeScope {}

/// This is ok because scope will only ever be used by the dispatcher thread.
unsafe impl Sync for UnsafeScope {}

impl UnsafeScope {
    /// Wrap a dynamic RawScope to make it Send + Sync.
    pub fn new(scope: Rc<OutputScope + 'static>) -> Self {
        UnsafeScope(scope)
    }
}

impl ops::Deref for UnsafeScope {
    type Target = OutputScope + 'static;
    fn deref(&self) -> &Self::Target {
        Rc::as_ref(&self.0)
    }
}

impl fmt::Debug for OutputMetric {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Box<Fn(Value)>")
    }
}

unsafe impl Send for OutputMetric {}

unsafe impl Sync for OutputMetric {}
