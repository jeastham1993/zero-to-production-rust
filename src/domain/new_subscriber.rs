use crate::domain::subscriber_email::SubscriberEmail;
use crate::domain::subscriber_name::SubscriberName;
use tracing::log::info;

pub struct NewSubscriber {
    pub email: SubscriberEmail,
    pub name: SubscriberName,
}

pub struct ConfirmedSubscriber {
    pub email: SubscriberEmail,
}
