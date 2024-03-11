use std::cell::RefCell;

use crate::dyn_method::DynMethod;
use crate::ty::Ty;
use crate::{Eventstore, Method, Provider};

struct On<F> {
    event: &'static str,
    handler: RefCell<Option<DynMethod<()>>>,
    method: F,
}

impl<F, P> Method<P> for On<F>
where
    F: Method<P>,
{
    type Output = F::Output;

    #[inline(always)]
    fn display_name(&self) -> Option<String> {
        self.method.display_name()
    }

    #[inline(always)]
    fn events(&self) -> Option<Eventstore> {
        if let Some(handler) = self.handler.borrow_mut().take() {
            let mut events = self.method.events().unwrap_or_default();
            events.insert(self.event, handler);
            Some(events)
        } else {
            self.method.events()
        }
    }

    #[inline(always)]
    fn dependencies() -> Vec<Ty> {
        F::dependencies()
    }

    #[inline(always)]
    fn call(self, provider: &Provider) -> Self::Output {
        self.method.call(provider)
    }
}

#[inline(always)]
pub fn on<F, Fp, H, Hp>(
    method: F,
    event: &'static str,
    handler: H,
) -> impl Method<Fp, Output = F::Output>
where
    F: Method<Fp>,
    H: Method<Hp>,
{
    let handler = super::map::map(handler, |_| ());
    let handler = DynMethod::new(handler);
    On {
        event,
        handler: RefCell::new(Some(handler)),
        method,
    }
}
