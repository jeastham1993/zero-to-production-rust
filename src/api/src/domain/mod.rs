pub mod new_subscriber;
mod newsletter_metadata;
mod newsletter_store;
pub mod subscriber_email;
pub mod subscriber_name;
pub mod subscriber_repository;

pub use crate::domain::newsletter_metadata::NewsletterMetadata;
pub use crate::domain::newsletter_store::{NewsletterStore, NewsletterStoreError};
