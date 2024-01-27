use std::{future::{ready, Future, Ready}, io};
use std::pin::Pin;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::Error;

pub struct TraceData;

impl<S, B> Transform<S, ServiceRequest> for TraceData
    where
        S: Service<ServiceRequest, Response=ServiceResponse<B>, Error=Error>,
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
        S: Service<ServiceRequest, Response=ServiceResponse<B>, Error=Error>,
        S::Future: 'static,
        B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output=Result<Self::Response, Self::Error>>>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let provider = match self.get_provider(&req) {
            Some(provider) => provider,
            None => {
                let err = io::Error::new(io::ErrorKind::Other, "Couldn't get provider");
                return Box::pin(async { Err(Error::from(err)) })
            },
        };
        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
            provider.force_flush();
            Ok(res)
        })
    }
}

impl<S> TraceDataMiddleware<S> {
    fn get_provider(&self, req: &ServiceRequest) -> Option<opentelemetry_sdk::trace::TracerProvider> {
        req.app_data::<actix_web::web::Data<opentelemetry_sdk::trace::TracerProvider>>()
            .and_then(|data| Some(data.as_ref()))
            .cloned()
    }
}