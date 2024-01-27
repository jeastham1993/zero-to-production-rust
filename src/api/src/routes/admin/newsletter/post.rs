use crate::utils::error_chain_fmt;
use crate::utils::{see_other};
use actix_web::http::StatusCode;
use actix_web::{web, HttpResponse, ResponseError};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use crate::domain::{NewsletterMetadata, NewsletterStore};

#[derive(thiserror::Error)]
pub enum PublishNewsletterError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishNewsletterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishNewsletterError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
}

#[tracing::instrument(
skip(form, newsletter_store),
)]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    newsletter_store: web::Data<dyn NewsletterStore>
) -> Result<HttpResponse, PublishNewsletterError> {
    newsletter_store
        .store_newsletter_metadata(NewsletterMetadata::new(&form.title, &form.text_content, &form.html_content))
        .await
        .context("Failure storing newsletter data")?;

    FlashMessage::info("The newsletter issue has been published!").send();
    Ok(see_other("/admin/newsletters"))
}
