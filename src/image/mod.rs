use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use tokio::fs as async_fs;
use tracing::{info, debug};
use tar::Archive;
use flate2::read::GzDecoder;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub name: String,
    pub tag: String,
    pub layers: Vec<Layer>,
    pub config: ImageConfig,
    pub wasm_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub digest: String,
    pub size: u64,
    pub media_type: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    pub env: Vec<String>,
    pub cmd: Vec<String>,
    pub entrypoint: Vec<String>,
    pub workdir: String,
    pub exposed_ports: HashMap<String, PortConfig>,
    pub volumes: HashMap<String, VolumeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConfig {
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeConfig {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OCIManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub config: OCIDescriptor,
    pub layers: Vec<OCIDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OCIDescriptor {
    pub digest: String,
    pub size: u64,
    #[serde(rename = "mediaType")]
    pub media_type: String,
}

pub struct ImageManager {
    cache_dir: PathBuf,
}

impl ImageManager {
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow!("Could not determine cache directory"))?
            .join("wasm-container")
            .join("images");
        
        fs::create_dir_all(&cache_dir)?;
        
        Ok(Self { cache_dir })
    }
    
    pub async fn get_or_pull(&self, image_ref: &str) -> Result<ImageData> {
        let (name, tag) = self.parse_image_ref(image_ref)?;
        
        if let Ok(image) = self.load_from_cache(&name, &tag).await {
            info!("Using cached image: {}:{}", name, tag);
            return Ok(image);
        }
        
        info!("Image not found in cache, pulling: {}:{}", name, tag);
        self.pull(image_ref).await
    }
    
    pub async fn pull(&self, image_ref: &str) -> Result<ImageData> {
        let (name, tag) = self.parse_image_ref(image_ref)?;
        
        info!("Pulling image: {}:{}", name, tag);
        
        let image_dir = self.cache_dir.join(&name).join(&tag);
        async_fs::create_dir_all(&image_dir).await?;
        
        let manifest = self.fetch_manifest(&name, &tag).await?;
        
        let config = self.fetch_config(&name, &manifest.config).await?;
        
        let mut layers = Vec::new();
        for layer_desc in &manifest.layers {
            let layer = self.fetch_layer(&name, layer_desc, &image_dir).await?;
            layers.push(layer);
        }
        
        let wasm_path = self.extract_wasm_binary(&image_dir, &layers).await?;
        
        let image_data = ImageData {
            name: name.clone(),
            tag: tag.clone(),
            layers,
            config,
            wasm_path,
        };
        
        self.save_to_cache(&image_data).await?;
        
        Ok(image_data)
    }
    
    fn parse_image_ref(&self, image_ref: &str) -> Result<(String, String)> {
        let parts: Vec<&str> = image_ref.split(':').collect();
        
        let (name, tag) = match parts.len() {
            1 => (parts[0].to_string(), "latest".to_string()),
            2 => (parts[0].to_string(), parts[1].to_string()),
            _ => return Err(anyhow!("Invalid image reference: {}", image_ref)),
        };
        
        Ok((name, tag))
    }
    
    async fn fetch_manifest(&self, _name: &str, _tag: &str) -> Result<OCIManifest> {
        Ok(OCIManifest {
            schema_version: 2,
            config: OCIDescriptor {
                digest: "sha256:mock".to_string(),
                size: 1024,
                media_type: "application/vnd.oci.image.config.v1+json".to_string(),
            },
            layers: vec![
                OCIDescriptor {
                    digest: "sha256:layer1".to_string(),
                    size: 2048,
                    media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
                },
            ],
        })
    }
    
    async fn fetch_config(&self, _name: &str, _config_desc: &OCIDescriptor) -> Result<ImageConfig> {
        Ok(ImageConfig {
            env: vec!["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string()],
            cmd: vec!["/bin/sh".to_string()],
            entrypoint: vec![],
            workdir: "/".to_string(),
            exposed_ports: HashMap::new(),
            volumes: HashMap::new(),
        })
    }
    
    async fn fetch_layer(&self, _name: &str, layer_desc: &OCIDescriptor, image_dir: &Path) -> Result<Layer> {
        let layer_path = image_dir.join(format!("{}.tar.gz", layer_desc.digest.replace("sha256:", "")));
        
        let demo_tar = vec![0u8; 1024];
        async_fs::write(&layer_path, demo_tar).await?;
        
        Ok(Layer {
            digest: layer_desc.digest.clone(),
            size: layer_desc.size,
            media_type: layer_desc.media_type.clone(),
            path: layer_path,
        })
    }
    
    async fn extract_wasm_binary(&self, image_dir: &Path, _layers: &[Layer]) -> Result<Option<PathBuf>> {
        let wasm_path = image_dir.join("app.wasm");
        
        let demo_wasm = include_bytes!("demo.wasm");
        async_fs::write(&wasm_path, demo_wasm).await?;
        
        Ok(Some(wasm_path))
    }
    
    async fn load_from_cache(&self, name: &str, tag: &str) -> Result<ImageData> {
        let cache_file = self.cache_dir.join(&name).join(&tag).join("metadata.json");
        
        if !cache_file.exists() {
            return Err(anyhow!("Image not found in cache"));
        }
        
        let metadata = async_fs::read_to_string(&cache_file).await?;
        let image_data: ImageData = serde_json::from_str(&metadata)?;
        
        Ok(image_data)
    }
    
    async fn save_to_cache(&self, image_data: &ImageData) -> Result<()> {
        let cache_file = self.cache_dir
            .join(&image_data.name)
            .join(&image_data.tag)
            .join("metadata.json");
        
        let metadata = serde_json::to_string_pretty(image_data)?;
        async_fs::write(&cache_file, metadata).await?;
        
        Ok(())
    }
}

impl ImageData {
    pub async fn get_wasm_binary(&self) -> Result<Vec<u8>> {
        if let Some(wasm_path) = &self.wasm_path {
            let wasm_bytes = async_fs::read(wasm_path).await?;
            Ok(wasm_bytes)
        } else {
            Err(anyhow!("No WASM binary found in image"))
        }
    }
}