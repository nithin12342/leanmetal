//! packages/edge-guard-sdk/src/lib.rs
//! Zero-Overhead In-Process Feature Guard Trait Contracts and Middleware.

use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use fast_rule_evaluator::{evaluate_flag, EvaluationContext, EvaluationResult, FlagDefinition};
use http::StatusCode;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Zero-cost abstract interface for feature flag guards.
pub trait FlagGuard: Send + Sync + 'static {
    /// Inlined boolean check returning true if the flag is enabled for this context.
    fn is_enabled(&self, flag_key: &str, context: &EvaluationContext) -> bool;

    /// Evaluates the complete flag outcome in-process without network hops.
    fn evaluate(&self, flag_key: &str, context: &EvaluationContext) -> EvaluationResult;
}

/// In-memory static guard implementation for fast evaluation.
#[derive(Default)]
pub struct MemoryFlagGuard {
    flags: RwLock<HashMap<String, FlagDefinition>>,
}

impl MemoryFlagGuard {
    pub fn new() -> Self {
        Self {
            flags: RwLock::new(HashMap::new()),
        }
    }

    pub fn set_flag(&self, flag: FlagDefinition) {
        let mut map = self.flags.write().unwrap();
        map.insert(flag.key.clone(), flag);
    }
}

impl FlagGuard for MemoryFlagGuard {
    #[inline(always)]
    fn is_enabled(&self, flag_key: &str, context: &EvaluationContext) -> bool {
        let map = self.flags.read().unwrap();
        if let Some(flag) = map.get(flag_key) {
            evaluate_flag(flag, context).enabled
        } else {
            false
        }
    }

    #[inline(always)]
    fn evaluate(&self, flag_key: &str, context: &EvaluationContext) -> EvaluationResult {
        let map = self.flags.read().unwrap();
        if let Some(flag) = map.get(flag_key) {
            evaluate_flag(flag, context)
        } else {
            EvaluationResult {
                flag_key: flag_key.to_string(),
                enabled: false,
                variant: None,
                matched_rule_index: None,
                reason: "flag_not_found".to_string(),
            }
        }
    }
}

/// Tower layer wrapping arbitrary services with a feature flag guard check.
#[derive(Clone)]
pub struct EdgeFlagLayer<G: FlagGuard> {
    guard: Arc<G>,
    flag_key: String,
}

impl<G: FlagGuard> EdgeFlagLayer<G> {
    pub fn new(guard: Arc<G>, flag_key: impl Into<String>) -> Self {
        Self {
            guard,
            flag_key: flag_key.into(),
        }
    }
}

impl<S, G: FlagGuard> Layer<S> for EdgeFlagLayer<G> {
    type Service = EdgeFlagService<S, G>;

    fn layer(&self, inner: S) -> Self::Service {
        EdgeFlagService {
            inner,
            guard: self.guard.clone(),
            flag_key: self.flag_key.clone(),
        }
    }
}

/// Tower service executing inlined flag evaluation before dispatching requests.
#[derive(Clone)]
pub struct EdgeFlagService<S, G: FlagGuard> {
    inner: S,
    guard: Arc<G>,
    flag_key: String,
}

impl<S, G, B> Service<Request<B>> for EdgeFlagService<S, G>
where
    S: Service<Request<B>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    G: FlagGuard,
    B: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let user_id = req
            .headers()
            .get("x-user-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("anonymous")
            .to_string();

        let ctx = EvaluationContext {
            user_id,
            attributes: Default::default(),
        };

        if !self.guard.is_enabled(&self.flag_key, &ctx) {
            let resp = (StatusCode::FORBIDDEN, "Feature flag disabled for context").into_response();
            return Box::pin(async move { Ok(resp) });
        }

        let fut = self.inner.call(req);
        Box::pin(async move { fut.await })
    }
}
