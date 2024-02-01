
use zero2prod::configuration::get_configuration;
use zero2prod::startup::Application;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let configuration = get_configuration().await.expect("Failed to read configuration");

    let application = Application::build(configuration).await?;
    application.run_until_stopped().await?;

    Ok(())
}
