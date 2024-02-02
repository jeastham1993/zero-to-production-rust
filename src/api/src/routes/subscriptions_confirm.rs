use crate::domain::subscriber_repository::SubscriberRepository;
use crate::utils::error_chain_fmt;
use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use reqwest::StatusCode;

#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[derive(thiserror::Error)]
pub enum ConfirmationError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
    #[error("There is no subscriber associated with the provided token.")]
    UnknownToken,
}

impl std::fmt::Debug for ConfirmationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ConfirmationError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::UnknownToken => StatusCode::UNAUTHORIZED,
            Self::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(name = "confirm_subscriber", skip(parameters, repo), fields())]
pub async fn confirm(
    parameters: web::Query<Parameters>,
    repo: web::Data<dyn SubscriberRepository + Send + Sync>,
) -> Result<HttpResponse, ConfirmationError> {
    let id = repo
        .get_subscriber_id_from_token(&parameters.subscription_token)
        .await
        .context("Failed to retrieve subscription token")?;

    match id {
        None => Err(ConfirmationError::UnknownToken),
        Some(subscriber_id) => {
            repo.confirm_subscriber(subscriber_id)
                .await
                .context("Failed to confirm subscriber")?;

            Ok(HttpResponse::Ok().finish())
        }
    }
}
