//! Chain of command for unscoped metrics.

use core::*;
use scope::MetricScope;

use namespace::{WithNamespace, add_namespace, Namespace};
use std::sync::{Arc, RwLock};

use scope::DefineMetric;
use output;

lazy_static! {
    /// The reference instance identifying an uninitialized metric config.
    pub static ref NO_METRIC_CONTEXT: Arc<OpenScope + Send + Sync> = Arc::new(output::to_void());

    /// The global instance to open scopes from if no other has been specified.
    pub static ref DEFAULT_CONTEXT: RwLock<Arc<OpenScope + Send + Sync>> = RwLock::new(NO_METRIC_CONTEXT.clone());
}

/// Wrap a MetricContext in a non-generic trait.
pub trait OpenScope {
    /// Open a new metrics scope
    fn open_scope(&self) -> Arc<DefineMetric + Send + Sync>;
}

/// Install a new receiver for all dispatched metrics, replacing any previous receiver.
pub fn default_aggregate_config<IS: Into<MetricContext<T>>, T: Send + Sync + Clone + 'static>(into_ctx: IS) {
    let ctx = Arc::new(into_ctx.into());
    *DEFAULT_CONTEXT.write().unwrap() = ctx;
}


/// A pair of functions composing a twin "chain of command".
/// This is the building block for the metrics backend.
#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct MetricContext<M> {
    #[derivative(Debug = "ignore")]
    define_metric_fn: DefineMetricFn<M>,

    #[derivative(Debug = "ignore")]
    open_scope_fn: OpenScopeFn<M>,
}

impl<M> MetricContext<M> {
    /// Open a new metric scope.
    /// Scope metrics allow an application to emit per-operation statistics,
    /// For example, producing a per-request performance log.
    ///
    /// Although the scope metrics can be predefined like in ['AppMetrics'], the application needs to
    /// create a scope that will be passed back when reporting scoped metric values.
    ///
    /// ```rust
    /// use dipstick::*;
    /// let scope_metrics = to_log().open_scope();
    /// let request_counter = scope_metrics.counter("scope_counter");
    /// ```
    ///
    pub fn open_scope(&self) -> MetricScope<M> {
        MetricScope::new(self.define_metric_fn.clone(), (self.open_scope_fn)())
    }

}

/// Create a new metric chain with the provided metric definition and scope creation functions.
pub fn metric_context<MF, WF, M>(define_fn: MF, open_scope_fn: WF) -> MetricContext<M>
    where
        MF: Fn(Kind, &str, Sampling) -> M + Send + Sync + 'static,
        WF: Fn() -> WriteFn<M> + Send + Sync + 'static,
{
    MetricContext {
        define_metric_fn: Arc::new(define_fn),
        open_scope_fn: Arc::new(open_scope_fn),
    }
}

impl<M: Send + Sync + Clone + 'static> MetricContext<M> {

    /// Intercept both metric definition and scope creation, possibly changing the metric type.
    pub fn wrap_all<MF, N>(&self, mod_fn: MF) -> MetricContext<N>
    where
        MF: Fn(DefineMetricFn<M>, OpenScopeFn<M>) -> (DefineMetricFn<N>, OpenScopeFn<N>),
        N: Clone + Send + Sync,
    {
        let (define_metric_fn, open_scope_fn) = mod_fn(
            self.define_metric_fn.clone(),
            self.open_scope_fn.clone()
        );
        MetricContext {
            define_metric_fn,
            open_scope_fn,
        }
    }

    /// Intercept scope creation.
    pub fn wrap_scope<MF>(&self, mod_fn: MF) -> Self
    where
        MF: Fn(OpenScopeFn<M>) -> OpenScopeFn<M>,
    {
        MetricContext {
            define_metric_fn: self.define_metric_fn.clone(),
            open_scope_fn: mod_fn(self.open_scope_fn.clone()),
        }
    }
}

impl<M: Send + Sync + Clone + 'static> OpenScope for MetricContext<M> {
    fn open_scope(&self) -> Arc<DefineMetric + Send + Sync> {
        Arc::new(self.open_scope())
    }
}

impl<M> From<MetricContext<M>> for MetricScope<M> {
    fn from(metrics: MetricContext<M>) -> MetricScope<M> {
        metrics.open_scope()
    }
}

impl<M: Send + Sync + Clone + 'static> WithNamespace for MetricContext<M> {
    fn with_name<IN: Into<Namespace>>(&self, names: IN) -> Self {
        let ref ninto = names.into();
        MetricContext {
            define_metric_fn: add_namespace(ninto, self.define_metric_fn.clone()),
            open_scope_fn: self.open_scope_fn.clone(),
        }
    }
}

