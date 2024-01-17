use std::cell::RefCell;
use std::rc::Rc;

use deno_core::{OpMetricsEvent, OpMetricsFactoryFn, OpMetricsFn};
use tracing::debug;

#[derive(Clone)]
pub struct Tape {
    pub feed: Rc<RefCell<Vec<Punch>>>,
}

impl Tape {
    pub fn new(hash: [u8; 32]) -> Self {
        Self {
            feed: Rc::new(RefCell::new(vec![Punch::Start(hash)])),
        }
    }

    /// Returns a [`OpMetricsFn`] for this tracker.
    pub fn op_metrics_fn(self) -> OpMetricsFn {
        Rc::new(move |ctx, event, _source| {
            let decl = ctx.decl();
            let name = decl.name.to_string();
            debug!("{name}");
            match event {
                OpMetricsEvent::Dispatched => {
                    let mut feed = self.feed.borrow_mut();
                    if decl.is_async {
                        feed.push(Punch::OpDispatchedAsync(name));
                    } else {
                        feed.push(Punch::OpDispatched(name))
                    }
                },
                OpMetricsEvent::Completed => {
                    let mut feed = self.feed.borrow_mut();
                    if decl.is_async {
                        feed.push(Punch::OpCompletedAsync(name));
                    } else {
                        feed.push(Punch::OpCompleted(name));
                    }
                },
                OpMetricsEvent::CompletedAsync => {
                    self.feed.borrow_mut().push(Punch::OpCompletedAsync(name))
                },
                OpMetricsEvent::Error => self.feed.borrow_mut().push(Punch::OpError(name)),
                OpMetricsEvent::ErrorAsync => {
                    self.feed.borrow_mut().push(Punch::OpErrorAsync(name))
                },
            }
        })
    }

    pub fn op_metrics_factory_fn(self) -> OpMetricsFactoryFn {
        Box::new(move |_, _, _| Some(self.clone().op_metrics_fn()))
    }

    /// Consumes the inner refcell and returns the feed
    pub fn end(self) -> Vec<Punch> {
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
    Start([u8; 32]),
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
        use std::fmt::Write;
        match self {
            Self::Start(arg0) => {
                let hex: String = arg0.iter().fold(String::with_capacity(64), |mut s, b| {
                    write!(s, "{b:02x}").unwrap();
                    s
                });
                f.debug_tuple("Start").field(&hex).finish()
            },
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
