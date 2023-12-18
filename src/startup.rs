use crate::routes::health_check::health_check;
use crate::routes::migrate::migrate_db;
use crate::routes::subscriptions::subscribe;
use crate::telemetry::CustomLevelRootSpanBuilder;
use actix_web::dev::{Server, Service};
use actix_web::{web, App, HttpMessage, HttpServer};
use reqwest::header::{HeaderName, HeaderValue};
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::{RequestId, TracingLogger};

pub fn run(listener: TcpListener, connection: PgPool) -> Result<Server, std::io::Error> {
    let connection = web::Data::new(connection);

    let server = HttpServer::new(move || {
        App::new()
            .route("/health_check", web::get().to(health_check))
            .wrap_fn(|req, srv| {
                let request_id = req.extensions().get::<RequestId>().copied();
                let res = srv.call(req);
                async move {
                    let mut res = res.await?;
                    if let Some(request_id) = request_id {
                        res.headers_mut().insert(
                            HeaderName::from_static("x-request-id"),
                            // this unwrap never fails, since UUIDs are valid ASCII strings
                            HeaderValue::from_str(&request_id.to_string()).unwrap(),
                        );
                    }
                    Ok(res)
                }
            })
            .wrap(TracingLogger::<CustomLevelRootSpanBuilder>::new())
            .route("/_migrate", web::get().to(migrate_db))
            .route("/subscriptions", web::post().to(subscribe))
            .app_data(connection.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
