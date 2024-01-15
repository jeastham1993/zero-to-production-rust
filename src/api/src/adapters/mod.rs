pub mod dynamo_db_session_store;
pub mod dynamodb_subscriber_repository;
pub mod dynamodb_user_repository;
pub mod postmark_email_client;
mod s3_newsletter_metadata_storage;

pub use crate::adapters::s3_newsletter_metadata_storage::S3NewsletterMetadataStorage;