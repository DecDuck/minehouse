use serde::Deserialize;

#[derive(Deserialize)]
pub struct WorkerConfig {
    pub control_endpoint: String,
    pub server_addr: String,
    pub account: Account,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Account {
    Offline(OfflineAccount),
    Online(OnlineAccount),
    Drasl(DraslAccount),
}

#[derive(Deserialize)]
pub struct DraslAccount {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct OfflineAccount {
    pub username: String,
}

#[derive(Deserialize)]
pub struct OnlineAccount {
    pub email: String,
}
pub fn load_config() -> Result<WorkerConfig, anyhow::Error> {
    let config_data = std::fs::read_to_string("./config.yaml")?;
    let config = serde_yaml2::from_str(&config_data)?;
    Ok(config)
}