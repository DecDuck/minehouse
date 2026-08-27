use azalea::account::Account as AzaleaAccount;
use serde::Deserialize;

use crate::drasl;

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

impl Account {
    pub async fn signin(self) -> Result<AzaleaAccount, anyhow::Error> {
        match self {
            Account::Offline(offline_account) => {
                Ok(AzaleaAccount::offline(&offline_account.username))
            }
            Account::Online(online_account) => {
                Ok(AzaleaAccount::microsoft(&online_account.email).await?)
            }
            Account::Drasl(drasl_account) => {
                drasl::DraslAccount::authenticate(
                    &drasl_account.base_url,
                    &drasl_account.username,
                    &drasl_account.password,
                )
                .await
            }
        }
    }
}
