//! Netflix Eureka service registration and discovery client

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EurekaError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("service not found: {0}")]
    ServiceNotFound(String),
    #[error("registration failed: {0}")]
    RegistrationFailed(String),
}

pub type Result<T> = std::result::Result<T, EurekaError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfo {
    pub instance_id: String,
    pub app_name: String,
    pub ip_addr: String,
    pub port: PortInfo,
    pub status: InstanceStatus,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    #[serde(rename = "$")]
    pub port: u16,
    #[serde(rename = "@enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstanceStatus {
    UP,
    DOWN,
    STARTING,
    OUT_OF_SERVICE,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Application {
    pub name: String,
    pub instance: Vec<InstanceInfo>,
}

#[derive(Debug, Clone)]
pub struct EurekaConfig {
    pub server_url: String,
    pub app_name: String,
    pub instance_id: String,
    pub ip_address: String,
    pub port: u16,
    pub renewal_interval: Duration,
    pub fetch_interval: Duration,
}

impl Default for EurekaConfig {
    fn default() -> Self {
        Self {
            server_url: "http://127.0.0.1:8761".into(),
            app_name: "unknown".into(),
            instance_id: "unknown".into(),
            ip_address: "127.0.0.1".into(),
            port: 8080,
            renewal_interval: Duration::from_secs(30),
            fetch_interval: Duration::from_secs(30),
        }
    }
}

pub struct EurekaClient {
    config: EurekaConfig,
    http: reqwest::Client,
}

impl EurekaClient {
    pub fn new(config: EurekaConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;
        Ok(Self { config, http })
    }

    fn app_url(&self, app: &str) -> String {
        format!("{}/eureka/apps/{}", self.config.server_url, app)
    }

    pub async fn register(&self, instance: &InstanceInfo) -> Result<()> {
        let url = self.app_url(&instance.app_name);
        self.http.post(&url)
            .header("Content-Type", "application/json")
            .json(instance)
            .send().await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn deregister(&self, app_name: &str, instance_id: &str) -> Result<()> {
        let url = format!("{}/eureka/apps/{}/{}", self.config.server_url, app_name, instance_id);
        self.http.delete(&url).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn heartbeat(&self, app_name: &str, instance_id: &str) -> Result<()> {
        let url = format!("{}/eureka/apps/{}/{}", self.config.server_url, app_name, instance_id);
        self.http.put(&url).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn get_application(&self, app_name: &str) -> Result<Application> {
        let url = self.app_url(app_name);
        let resp = self.http.get(&url)
            .header("Accept", "application/json")
            .send().await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(EurekaError::ServiceNotFound(app_name.into()));
        }
        resp.error_for_status()?.json().await.map_err(Into::into)
    }

    pub async fn get_all_applications(&self) -> Result<Vec<Application>> {
        let url = format!("{}/eureka/apps", self.config.server_url);
        let resp = self.http.get(&url)
            .header("Accept", "application/json")
            .send().await?
            .error_for_status()?;
        let apps: serde_json::Value = resp.json().await?;
        let applications = apps.get("applications").and_then(|a| a.get("application"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        Ok(applications)
    }

    pub async fn health(&self) -> Result<bool> {
        let url = format!("{}/eureka/status", self.config.server_url);
        let resp = self.http.get(&url).send().await?;
        Ok(resp.status().is_success())
    }
}
