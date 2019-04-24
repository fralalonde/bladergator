//! Metric input scope caching.

use core::attributes::{Attributes, OnFlush, Prefixed, WithAttributes};
use core::error;
use core::input::{Input, InputDyn, InputKind, InputMetric, InputScope};
use core::name::MetricName;
use core::Flush;

use std::sync::Arc;

#[cfg(not(feature = "parking_lot"))]
use std::sync::RwLock;

#[cfg(feature = "parking_lot")]
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicIsize};
use std::sync::atomic::Ordering::Relaxed;


pub trait Gapless: Input + Send + Sync + 'static + Sized {
    fn gapless(self) -> GaplessInput {
        GaplessInput::wrap(self)
    }
}

/// Output wrapper caching frequently defined metrics
#[derive(Clone)]
pub struct GaplessInput {
    attributes: Attributes,
    target: Arc<InputDyn + Send + Sync + 'static>,
}

impl GaplessInput {
    /// Wrap scopes with an asynchronous metric write & flush dispatcher.
    fn wrap<OUT: Input + Send + Sync + 'static>(target: OUT) -> GaplessInput {
        GaplessInput {
            attributes: Attributes::default(),
            target: Arc::new(target),
        }
    }
}

impl WithAttributes for GaplessInput {
    fn get_attributes(&self) -> &Attributes {
        &self.attributes
    }
    fn mut_attributes(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
}

impl Input for GaplessInput {
    type SCOPE = GaplessInputScope;

    fn metrics(&self) -> Self::SCOPE {
        let target = self.target.input_dyn();
        GaplessInputScope {
            attributes: self.attributes.clone(),
            target,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

struct LastValueMetric {
    metric: InputMetric,
    touched: AtomicBool,
    last_value: AtomicIsize,
}

/// Input wrapper caching frequently defined metrics
#[derive(Clone)]
pub struct GaplessInputScope {
    attributes: Attributes,
    target: Arc<InputScope + Send + Sync + 'static>,
    cache: Arc<RwLock<HashMap<MetricName, Arc<LastValueMetric>>>>,
}

impl WithAttributes for GaplessInputScope {
    fn get_attributes(&self) -> &Attributes {
        &self.attributes
    }
    fn mut_attributes(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
}

impl InputScope for GaplessInputScope {
    fn new_metric(&self, name: MetricName, kind: InputKind) -> InputMetric {
        let name = self.prefix_append(name);
        let lookup = { write_lock!(self.cache).get(&name).cloned() };
        let last_val_metric: Arc<LastValueMetric> = lookup.unwrap_or_else(|| {
            let new_metric = Arc::new(LastValueMetric{
                metric: self.target.new_metric(name.clone(), kind),
                touched: AtomicBool::new(false),
                last_value: AtomicIsize::new(0),
            });
            // FIXME (perf) having to take another write lock for a cache miss
            write_lock!(self.cache).insert(name, new_metric.clone());
            new_metric
        });
        InputMetric::new(move |value, labels| {
            last_val_metric.last_value.store(value, Relaxed);
            last_val_metric.touched.store(true, Relaxed);
            last_val_metric.metric.write(value, labels)
        })
    }
}

impl Flush for GaplessInputScope {
    fn flush(&self) -> error::Result<()> {
        self.notify_flush_listeners();
        let cache = read_lock!(self.cache);
        for last_val in cache.values() {
            if !last_val.touched.swap(false, Relaxed) {
                last_val.metric.write(last_val.last_value.load(Relaxed), labels!())
            }
        }
        self.target.flush()
    }
}


//#[cfg(test)]
//pub mod test {
//    use super::*;
//    use std::sync::atomic::AtomicUsize;
//    use output::map;
//
//    #[test]
//    fn fill_blanks() {
//        let map = map::StatsMap::default();
//        let metrics = map.gapless().metrics();
//
//        let counter = metrics.counter("count");
//        assert_eq!(None, map.as_map().get("count"));
//        counter.count(1);
//
//        assert_eq!(3, trig1a.load(SeqCst));
//    }
//
//}
