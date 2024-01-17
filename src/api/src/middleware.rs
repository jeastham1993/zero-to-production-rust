use std::{
    future::{ready, Future, Ready},
    marker::PhantomData,
    task::{Context, Poll},
};
use std::pin::Pin;
use std::process::Output;
use actix_web::dev::{Response, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::Error;

pub struct TraceData;

impl<S, B> Transform<S, ServiceRequest> for TraceData
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
        S::Future: 'static,
        B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = TraceDataMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TraceDataMiddleware { service }))
    }
}

pub struct TraceDataMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for TraceDataMiddleware<S>
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
        S::Future: 'static,
        B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let app_data = req.app_data::<actix_web::web::Data<opentelemetry_sdk::trace::TracerProvider>>();
        let state = app_data.as_ref().unwrap();
        let provider = state.as_ref().clone();
        let fut = self.service.call(req);

        Box::pin(async move {
            let mut res = fut.await?;

            provider.force_flush();

            Ok(res)
        })
    }
}