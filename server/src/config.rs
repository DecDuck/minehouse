use serde::Deserialize;


#[derive(Deserialize)]
pub struct MinehouseConfig {
    pub api_bind_addr: String,
    pub control_bind_addr: String,
    pub db_url: String,
}

pub async fn load_config() -> Result<MinehouseConfig, anyhow::Error> {
    let config_path = tokio::fs::read_to_string("./config.yaml").await?;
    let config: MinehouseConfig = serde_yaml2::from_str(&config_path)?;

    Ok(config)
}