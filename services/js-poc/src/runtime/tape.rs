use std::cell::RefCell;
use std::rc::Rc;

use deno_core::url::Url;
use deno_core::{OpMetricsEvent, OpMetricsFactoryFn, OpMetricsFn};

#[derive(Clone)]
pub struct Tape {
    pub feed: Rc<RefCell<Vec<Punch>>>,
}

impl Tape {
    pub fn new(hash: Url) -> Self {
        Self {
            feed: Rc::new(RefCell::new(vec![Punch::Start(hash)])),
        }
    }

    /// Returns a [`OpMetricsFn`] for this tracker.
    pub fn op_metrics_fn(&self) -> OpMetricsFn {
        let feed = self.feed.clone();
        Rc::new(move |ctx, event, _source| {
            let decl = ctx.decl();
            let name = decl.name.to_string();
            let mut feed = feed.borrow_mut();

            match event {
                OpMetricsEvent::Dispatched => feed.push(match decl.is_async {
                    true => Punch::OpDispatchedAsync(name),
                    false => Punch::OpDispatched(name),
                }),
                OpMetricsEvent::Completed => feed.push(match decl.is_async {
                    true => Punch::OpCompletedAsync(name),
                    false => Punch::OpCompleted(name),
                }),
                OpMetricsEvent::Error => feed.push(Punch::OpError(name)),
                OpMetricsEvent::CompletedAsync => feed.push(Punch::OpCompletedAsync(name)),
                OpMetricsEvent::ErrorAsync => feed.push(Punch::OpErrorAsync(name)),
            }
        })
    }

    pub fn op_metrics_factory_fn(&self) -> OpMetricsFactoryFn {
        let s = self.clone();
        Box::new(move |_, _, _| Some(s.op_metrics_fn()))
    }

    /// Consumes the inner refcell and returns the feed
    pub fn end(&self) -> Vec<Punch> {
        let mut feed = self.feed.take();
        feed.push(Punch::End);
        feed
    }
}

impl std::fmt::Debug for Tape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let feed = self.feed.borrow();
        f.debug_struct("Tape").field("feed", &feed).finish()
    }
}

#[derive(Clone)]
pub enum Punch {
    Start(Url),
    OpDispatched(String),
    OpCompleted(String),
    OpError(String),
    OpDispatchedAsync(String),
    OpCompletedAsync(String),
    OpErrorAsync(String),
    End,
}

impl std::fmt::Debug for Punch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start(arg0) => f.debug_tuple("Start").field(arg0).finish(),
            Self::OpDispatched(arg0) => f.debug_tuple("OpDispatched").field(arg0).finish(),
            Self::OpCompleted(arg0) => f.debug_tuple("OpCompleted").field(arg0).finish(),
            Self::OpError(arg0) => f.debug_tuple("OpError").field(arg0).finish(),
            Self::OpDispatchedAsync(arg0) => {
                f.debug_tuple("OpDispatchedAsync").field(arg0).finish()
            },
            Self::OpCompletedAsync(arg0) => f.debug_tuple("OpCompletedAsync").field(arg0).finish(),
            Self::OpErrorAsync(arg0) => f.debug_tuple("OpErrorAsync").field(arg0).finish(),
            Self::End => write!(f, "End"),
        }
    }
}
