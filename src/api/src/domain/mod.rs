pub mod new_subscriber;
pub mod subscriber_email;
pub mod subscriber_name;
pub mod subscriber_repository;
mod newsletter_store;
mod newsletter_metadata;


pub use crate::domain::newsletter_store::{NewsletterStoreError, NewsletterStore};
pub use crate::domain::newsletter_metadata::{NewsletterMetadata};
