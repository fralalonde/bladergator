//! Dispatch metrics to multiple sinks.

use core::Flush;
use core::input::{InputKind, Input, InputScope, InputMetric, InputDyn};
use core::attributes::{Attributes, WithAttributes, Prefixed};
use core::name::MetricName;
use core::error;

use std::sync::Arc;

/// Opens multiple scopes at a time from just as many outputs.
#[derive(Clone)]
pub struct MultiInput {
    attributes: Attributes,
    outputs: Vec<Arc<InputDyn + Send + Sync>>,
}

impl Input for MultiInput {
    type SCOPE = MultiInputScope;

    fn input(&self) -> Self::SCOPE {
        let scopes = self.outputs.iter().map(|out| out.input_dyn()).collect();
        MultiInputScope {
            attributes: self.attributes.clone(),
            scopes,
        }
    }
}

impl MultiInput {

    /// Create a new multi-output.
    pub fn input() -> MultiInput {
        MultiInput {
            attributes: Attributes::default(),
            outputs: vec![],
        }
    }

    /// Returns a clone of the dispatch with the new output added to the list.
    pub fn add_target<OUT: Input + Send + Sync + 'static>(&self, out: OUT) -> Self {
        let mut cloned = self.clone();
        cloned.outputs.push(Arc::new(out));
        cloned
    }
}

impl WithAttributes for MultiInput {
    fn get_attributes(&self) -> &Attributes { &self.attributes }
    fn mut_attributes(&mut self) -> &mut Attributes { &mut self.attributes }
}

/// Dispatch metric values to a list of scopes.
#[derive(Clone, Default)]
pub struct MultiInputScope {
    attributes: Attributes,
    scopes: Vec<Arc<InputScope + Send + Sync>>,
}

impl MultiInputScope {
    /// Create a new multi scope dispatcher with no scopes.
    pub fn new() -> Self {
        MultiInputScope {
            attributes: Attributes::default(),
            scopes: vec![],
        }
    }

    /// Returns a clone of the dispatch with the new output added to the list.
    pub fn add_target<IN: InputScope + Send + Sync + 'static>(&self, scope: IN) -> Self {
        let mut cloned = self.clone();
        cloned.scopes.push(Arc::new(scope));
        cloned
    }
}

impl InputScope for MultiInputScope {
    fn new_metric(&self, name: MetricName, kind: InputKind) -> InputMetric {
        let name = &self.prefix_append(name);
        let metrics: Vec<InputMetric> = self.scopes.iter()
            .map(move |scope| scope.new_metric(name.clone(), kind))
            .collect();
        InputMetric::new(move |value, labels| for metric in &metrics {
            metric.write(value, labels.clone())
        })
    }
}

impl Flush for MultiInputScope {
    fn flush(&self) -> error::Result<()> {
        for w in &self.scopes {
            w.flush()?;
        }
        Ok(())
    }
}

impl WithAttributes for MultiInputScope {
    fn get_attributes(&self) -> &Attributes { &self.attributes }
    fn mut_attributes(&mut self) -> &mut Attributes { &mut self.attributes }
}
