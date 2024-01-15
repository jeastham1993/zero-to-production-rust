use serde::{Deserialize, Serialize};


#[derive(Deserialize, Serialize)]
pub struct NewsletterMetadata {
    pub issue_title: String,
    pub text_content: String,
    pub html_content: String
}

impl NewsletterMetadata {
    pub fn new(issue_title: &str, text_content: &str, html_content: &str) -> Self{
        Self {
            issue_title: issue_title.to_string(),
            text_content: text_content.to_string(),
            html_content: html_content.to_string(),
        }
    }
}