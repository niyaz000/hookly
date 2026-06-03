use std::task::{Context, Poll};

use axum::http::Request;
use tower::{Layer, Service};

/// Request extension carrying the handler function name, injected per-route.
#[derive(Clone, Debug)]
pub struct HandlerName(pub &'static str);

/// Tower layer that injects a [`HandlerName`] extension into every request that passes through it.
/// Apply it with `.layer(SetHandlerName::of(&handler_fn))` on each `MethodRouter`.
#[derive(Clone)]
pub struct SetHandlerName {
    name: &'static str,
}

impl SetHandlerName {
    /// Captures the handler function name at compile time via `std::any::type_name`.
    /// Pass a reference to the handler function: `SetHandlerName::of(&my_handler)`.
    /// The last path segment of the fully-qualified type name is used (e.g. `"list_organizations"`).
    pub fn of<H>(_handler: &H) -> Self {
        let full: &'static str = std::any::type_name::<H>();
        let name: &'static str = full.rsplit("::").next().unwrap_or(full);
        Self { name }
    }
}

impl<S> Layer<S> for SetHandlerName {
    type Service = SetHandlerNameService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SetHandlerNameService { inner, name: self.name }
    }
}

#[derive(Clone)]
pub struct SetHandlerNameService<S> {
    inner: S,
    name: &'static str,
}

impl<S, B> Service<Request<B>> for SetHandlerNameService<S>
where
    S: Service<Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        req.extensions_mut().insert(HandlerName(self.name));
        self.inner.call(req)
    }
}
