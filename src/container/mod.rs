use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use crate::image::ImageData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub image: String,
    pub status: String,
}

#[derive(Debug)]
pub struct Container {
    id: String,
    image: ImageData,
    command: Option<Vec<String>>,
    workdir: Option<String>,
    env_vars: HashMap<String, String>,
    volumes: Vec<VolumeMount>,
    network_config: NetworkConfig,
}

#[derive(Debug)]
pub struct VolumeMount {
    pub host_path: PathBuf,
    pub container_path: PathBuf,
    pub read_only: bool,
}

#[derive(Debug)]
pub struct NetworkConfig {
    pub hostname: String,
    pub ports: Vec<PortMapping>,
}

#[derive(Debug, Clone)]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String,
}

impl Container {
    pub fn new(
        image: ImageData,
        command: Option<Vec<String>>,
        workdir: Option<String>,
        env: Vec<String>,
    ) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        
        let mut env_vars = HashMap::new();
        for env_str in env {
            if let Some((key, value)) = env_str.split_once('=') {
                env_vars.insert(key.to_string(), value.to_string());
            }
        }
        
        env_vars.insert("HOSTNAME".to_string(), id.clone());
        env_vars.insert("PATH".to_string(), "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string());
        
        Ok(Self {
            id: id.clone(),
            image,
            command,
            workdir,
            env_vars,
            volumes: Vec::new(),
            network_config: NetworkConfig {
                hostname: id,
                ports: Vec::new(),
            },
        })
    }
    
    pub fn id(&self) -> &str {
        &self.id
    }
    
    pub fn image_name(&self) -> &str {
        &self.image.name
    }
    
    pub fn command(&self) -> Option<&Vec<String>> {
        self.command.as_ref()
    }
    
    pub fn workdir(&self) -> Option<&str> {
        self.workdir.as_deref()
    }
    
    pub fn env_vars(&self) -> &HashMap<String, String> {
        &self.env_vars
    }
    
    pub async fn get_wasm_binary(&self) -> Result<Vec<u8>> {
        self.image.get_wasm_binary().await
    }
    
    pub fn add_volume(&mut self, host_path: PathBuf, container_path: PathBuf, read_only: bool) {
        self.volumes.push(VolumeMount {
            host_path,
            container_path,
            read_only,
        });
    }
    
    pub fn add_port_mapping(&mut self, host_port: u16, container_port: u16, protocol: String) {
        self.network_config.ports.push(PortMapping {
            host_port,
            container_port,
            protocol,
        });
    }
    
    pub fn volumes(&self) -> &[VolumeMount] {
        &self.volumes
    }
    
    pub fn network_config(&self) -> &NetworkConfig {
        &self.network_config
    }
    
    pub fn image_data(&self) -> &ImageData {
        &self.image
    }
}