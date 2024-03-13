use aws_config::default_provider::credentials::DefaultCredentialsChain;
use aws_config::meta::region::RegionProviderChain;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_ssm::config::ProvideCredentials;
use aws_sdk_ssm::Client;
use aws_smithy_runtime::client::http::hyper_014::HyperClientBuilder;
use config::FileFormat;
use secrecy::Secret;
use serde::Deserialize;
use telemetry::TelemetrySettings;

#[derive(Deserialize, Clone)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub telemetry: TelemetrySettings,
    pub application: ApplicationSettings,
}

#[derive(Deserialize, Clone)]
pub struct ApplicationSettings {
    pub application_port: u16,
    pub host_name: String,
    pub base_url: String,
    pub hmac_secret: Secret<String>,
}

#[derive(Deserialize, Clone)]
pub struct DatabaseSettings {
    pub database_name: String,
    pub auth_database_name: String,
    pub use_local: bool,
    pub newsletter_storage_bucket: String,
}

pub async fn get_configuration() -> Result<Settings, config::ConfigError> {
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");

    match environment {
        Environment::Local => {
            let base_path = std::env::current_dir().expect("Failed to determine the current directory");

            let configuration_directory = base_path.join("configuration");
            
            let environment_filename = format!("{}.yaml", environment.as_str());

            // Init configuration reader
            let settings = config::Config::builder()
                .add_source(config::File::from(
                    configuration_directory.join("base.yaml"),
                ))
                .add_source(config::File::from(
                    configuration_directory.join(environment_filename),
                ))
                // Add in settings from environment variables (with a prefix of APP and '__' as separator)
                // E.g. `APP_APPLICATION__PORT=5001 would set `Settings.application.port`
                .add_source(
                    config::Environment::with_prefix("APP")
                        .prefix_separator("_")
                        .separator("__"),
                )
                .build()?;

            settings.try_deserialize::<Settings>()
        }
        Environment::Production => {
            let ssm = configure_ssm().await;
            let ssm_client = Client::from_conf(ssm);

            let parameter = ssm_client
                .get_parameter()
                .name(std::env::var("CONFIG_PARAMETER_NAME").unwrap())
                .send()
                .await
                .expect("Parameter retrieval not successful");

            // Init configuration reader
            let settings = config::Config::builder()
                .add_source(config::File::from_str(
                    parameter.parameter.unwrap().value.unwrap().as_str(),
                    FileFormat::Yaml,
                ))
                .add_source(
                    config::Environment::with_prefix("APP")
                        .prefix_separator("_")
                        .separator("__"),
                )
                .build()?;

            settings.try_deserialize::<Settings>()
        }
    }
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

async fn configure_ssm() -> aws_sdk_ssm::Config {
    let https_connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_webpki_roots()
        .https_or_http()
        .enable_http1()
        .enable_http2()
        .build();

    let hyper_client = HyperClientBuilder::new().build(https_connector);

    let region = RegionProviderChain::default_provider()
        .or_else(Region::new("us-east-1"))
        .region()
        .await
        .unwrap();

    let credentials = DefaultCredentialsChain::builder()
        .region(region.clone())
        .build()
        .await
        .provide_credentials()
        .await
        .unwrap();

    let conf_builder = aws_sdk_ssm::Config::builder()
        .behavior_version(BehaviorVersion::v2023_11_09())
        .credentials_provider(credentials.clone())
        .http_client(hyper_client.clone())
        .region(region.clone());

    conf_builder.build()
}
