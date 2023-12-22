use reqwest::Url;
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use sqlx::postgres::PgConnectOptions;
use std::time::Duration;

#[derive(Deserialize, Clone)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub telemetry: TelemetrySettings,
    pub application: ApplicationSettings,
    pub email_settings: EmailClientSettings,
}

#[derive(Deserialize, Clone)]
pub struct ApplicationSettings {
    pub application_port: u16,
    pub host_name: String,
    pub base_url: String,
}

#[derive(Deserialize, Clone)]
pub struct TelemetrySettings {
    pub otlp_endpoint: String,
    pub honeycomb_api_key: Secret<String>,
    pub dataset_name: String,
}

#[derive(Deserialize, Clone)]
pub struct EmailClientSettings {
    pub base_url: String,
    pub sender_email: String,
    pub authorization_token: Secret<String>,
    pub timeout_milliseconds: u64,
}

impl EmailClientSettings {
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_milliseconds)
    }
}

#[derive(Deserialize, Clone)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    pub port: u16,
    pub host: String,
    pub database_name: String,
}

impl DatabaseSettings {
    pub fn without_db(&self) -> PgConnectOptions {
        PgConnectOptions::new()
            .host(&self.host)
            .username(&self.username)
            .password(self.password.expose_secret())
            .port(self.port)
    }

    pub fn with_db(&self) -> PgConnectOptions {
        self.without_db().database(&self.database_name)
    }

    pub fn connection_string(&self) -> Secret<String> {
        let db_url = format!(
            "postgres://{}:{}/{}",
            self.host, self.port, self.database_name
        );

        let mut uri = Url::parse(&db_url).unwrap();
        let _ = uri.set_username(&self.username);
        let _ = uri.set_password(Some(self.password.expose_secret()));

        Secret::new(uri.to_string())
    }

    pub fn connection_string_without_db(&self) -> Secret<String> {
        let db_url = format!("postgres://{}:{}", self.host, self.port);

        let mut uri = Url::parse(&db_url).unwrap();
        let _ = uri.set_username(&self.username);
        let _ = uri.set_password(Some(self.password.expose_secret()));

        Secret::new(uri.to_string())
    }
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");

    let configuration_directory = base_path.join("configuration");

    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");

    let environment_filename = format!("{}.yaml", environment.as_str());

    // Init configuration reader
    let settings = config::Config::builder()
        .add_source(config::File::from(
            configuration_directory.join("base.yaml"),
        ))
        .add_source(config::File::from(
            configuration_directory.join(environment_filename),
        ))
        .build()?;

    settings.try_deserialize::<Settings>()
}

pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a support environment. Use either local or production",
                other
            )),
        }
    }
}
