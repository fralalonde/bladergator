//! Dispatch metrics to multiple sinks.

use crate::core::attributes::{Attributes, OnFlush, Prefixed, WithAttributes, MetricId};
use crate::core::error;
use crate::core::input::InputKind;
use crate::core::name::MetricName;
use crate::core::output::{Output, OutputDyn, OutputMetric, OutputScope};
use crate::core::Flush;

use std::rc::Rc;
use std::sync::Arc;
use crate::{Locking, LockingOutput};

/// Opens multiple scopes at a time from just as many outputs.
#[derive(Clone, Default)]
pub struct MultiOutput {
    attributes: Attributes,
    outputs: Vec<Arc<dyn OutputDyn + Send + Sync + 'static>>,
}

impl Output for MultiOutput {
    type SCOPE = MultiOutputScope;

    fn new_scope(&self) -> Self::SCOPE {
        #[allow(clippy::redundant_closure)]
        let scopes = self.outputs.iter().map(|out| out.output_dyn()).collect();
        MultiOutputScope {
            attributes: self.attributes.clone(),
            scopes,
        }
    }
}

impl MultiOutput {
    /// Create a new multi-output dispatcher.
    #[deprecated(since = "0.7.2", note = "Use new()")]
    pub fn output() -> Self {
        Self::new()
    }

    /// Create a new multi-output dispatcher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a target to the dispatch list.
    /// Returns a clone of the original object.
    pub fn add_target<OUT: Output + Send + Sync + 'static>(&self, out: OUT) -> Self {
        let mut cloned = self.clone();
        cloned.outputs.push(Arc::new(out));
        cloned
    }
}

impl Locking for MultiOutput {
    fn locking(&self) -> LockingOutput {
        LockingOutput::new(self.get_attributes(), Rc::new(self.new_scope()))
    }
}

impl WithAttributes for MultiOutput {
    fn get_attributes(&self) -> &Attributes {
        &self.attributes
    }
    fn mut_attributes(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
}

/// Dispatch metric values to a list of scopes.
#[derive(Clone, Default)]
pub struct MultiOutputScope {
    attributes: Attributes,
    scopes: Vec<Rc<dyn OutputScope>>,
}

impl MultiOutputScope {
    /// Create a new multi scope dispatcher with no scopes.
    pub fn new() -> Self {
        MultiOutputScope {
            attributes: Attributes::default(),
            scopes: vec![],
        }
    }

    /// Returns a clone of the dispatch with the new output added to the list.
    pub fn add_target<IN: OutputScope + 'static>(&self, scope: IN) -> Self {
        let mut cloned = self.clone();
        cloned.scopes.push(Rc::new(scope));
        cloned
    }
}

impl OutputScope for MultiOutputScope {
    fn new_metric(&self, name: MetricName, kind: InputKind) -> OutputMetric {
        let mname = self.prefix_append(name.clone());
        let metrics: Vec<OutputMetric> = self
            .scopes
            .iter()
            .map(move |scope| scope.new_metric(mname.clone(), kind))
            .collect();
        OutputMetric::new(MetricId::forge("multi", name), move |value, labels| {
            for metric in &metrics {
                metric.write(value, labels.clone())
            }
        })
    }
}

impl Flush for MultiOutputScope {
    fn flush(&self) -> error::Result<()> {
        self.notify_flush_listeners();
        for w in &self.scopes {
            w.flush()?;
        }
        Ok(())
    }
}

impl WithAttributes for MultiOutputScope {
    fn get_attributes(&self) -> &Attributes {
        &self.attributes
    }
    fn mut_attributes(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
}
