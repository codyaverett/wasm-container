use anyhow::Result;
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::WasiCtxBuilder;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, debug};

use crate::container::{Container, ContainerInfo};
use crate::filesystem::Filesystem;
use crate::network::{NetworkManager, ContainerNetwork};

pub struct WasmRuntime {
    engine: Engine,
    containers: Arc<Mutex<Vec<ContainerInfo>>>,
    network_manager: NetworkManager,
}

impl WasmRuntime {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.wasm_threads(true);
        config.wasm_simd(true);
        config.async_support(true);
        
        let engine = Engine::new(&config)?;
        let network_manager = NetworkManager::new();
        
        Ok(Self {
            engine,
            containers: Arc::new(Mutex::new(Vec::new())),
            network_manager,
        })
    }
    
    pub async fn run(&mut self, mut container: Container) -> Result<()> {
        info!("Starting container: {}", container.id());
        
        let filesystem = Filesystem::new(&container)?;
        filesystem.setup().await?;
        
        let network = self.network_manager.setup_container_network(&container).await?;
        
        let wasi_ctx = self.build_wasi_context(&container, &filesystem, &network)?;
        
        let mut store = Store::new(&self.engine, wasi_ctx);
        
        let module = self.compile_container(&container).await?;
        
        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |s| s)?;
        
        self.add_custom_host_functions(&mut linker)?;
        
        let instance = linker.instantiate_async(&mut store, &module).await?;
        
        let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
        
        let container_info = ContainerInfo {
            id: container.id().to_string(),
            image: container.image_name().to_string(),
            status: "running".to_string(),
        };
        
        self.containers.lock().await.push(container_info);
        
        let result = start.call_async(&mut store, ()).await;
        
        self.network_manager.cleanup_container_network(container.id()).await?;
        
        match result {
            Ok(_) => {
                self.update_container_status(&container.id(), "exited").await?;
                info!("Container {} exited successfully", container.id());
            }
            Err(e) => {
                self.update_container_status(&container.id(), "failed").await?;
                info!("Container {} failed: {}", container.id(), e);
                return Err(e);
            }
        }
        
        Ok(())
    }
    
    pub async fn stop(&mut self, container_id: &str) -> Result<()> {
        self.update_container_status(container_id, "stopping").await?;
        self.network_manager.cleanup_container_network(container_id).await?;
        self.update_container_status(container_id, "stopped").await?;
        Ok(())
    }
    
    pub async fn list_containers(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let containers = self.containers.lock().await;
        
        if all {
            Ok(containers.clone())
        } else {
            Ok(containers
                .iter()
                .filter(|c| c.status == "running")
                .cloned()
                .collect())
        }
    }
    
    fn build_wasi_context(&self, container: &Container, filesystem: &Filesystem, network: &ContainerNetwork) -> Result<wasmtime_wasi::preview1::WasiP1Ctx> {
        let mut builder = WasiCtxBuilder::new();
        
        builder
            .inherit_stdio()
            .inherit_network();
        
        for (key, value) in container.env_vars() {
            builder.env(&key, &value);
        }
        
        builder.env("CONTAINER_IP", &network.get_ip().to_string());
        builder.env("HOSTNAME", network.get_hostname());
        
        use wasmtime_wasi::{DirPerms, FilePerms};
        
        if let Some(workdir) = container.workdir() {
            builder.preopened_dir(
                filesystem.rootfs_path().join(workdir.trim_start_matches('/')),
                "/",
                DirPerms::all(),
                FilePerms::all()
            )?;
        } else {
            builder.preopened_dir(
                filesystem.rootfs_path(),
                "/",
                DirPerms::all(),
                FilePerms::all()
            )?;
        }
        
        for volume in container.volumes() {
            filesystem.mount_volume(&volume.host_path, &volume.container_path)?;
        }
        
        if let Some(args) = container.command() {
            builder.args(&args);
        } else {
            let config = &container.image_data().config;
            if !config.entrypoint.is_empty() {
                let mut all_args = config.entrypoint.clone();
                all_args.extend(config.cmd.clone());
                builder.args(&all_args);
            } else if !config.cmd.is_empty() {
                builder.args(&config.cmd);
            }
        }
        
        Ok(builder.build_p1())
    }
    
    async fn compile_container(&self, container: &Container) -> Result<Module> {
        debug!("Compiling WASM module for container");
        
        let wasm_bytes = container.get_wasm_binary().await?;
        
        let module = Module::new(&self.engine, &wasm_bytes)?;
        
        Ok(module)
    }
    
    fn add_custom_host_functions(&self, linker: &mut Linker<wasmtime_wasi::preview1::WasiP1Ctx>) -> Result<()> {
        linker.func_wrap(
            "env",
            "container_log",
            |mut caller: wasmtime::Caller<'_, wasmtime_wasi::preview1::WasiP1Ctx>, ptr: i32, len: i32| -> wasmtime::Result<()> {
                let memory = caller.get_export("memory")
                    .and_then(|e| e.into_memory())
                    .ok_or_else(|| anyhow::anyhow!("failed to get memory"))?;
                
                let data = memory.data(&caller);
                if ptr < 0 || len < 0 || (ptr + len) as usize > data.len() {
                    return Err(anyhow::anyhow!("invalid memory access").into());
                }
                
                let message = std::str::from_utf8(&data[ptr as usize..(ptr + len) as usize])
                    .map_err(|_| anyhow::anyhow!("invalid UTF-8"))?;
                
                info!("[Container]: {}", message);
                
                Ok(())
            }
        )?;
        
        linker.func_wrap(
            "env", 
            "get_container_info",
            |_caller: wasmtime::Caller<'_, wasmtime_wasi::preview1::WasiP1Ctx>| -> wasmtime::Result<i32> {
                Ok(42)
            }
        )?;
        
        Ok(())
    }
    
    async fn update_container_status(&self, container_id: &str, status: &str) -> Result<()> {
        let mut containers = self.containers.lock().await;
        
        if let Some(container) = containers.iter_mut().find(|c| c.id == container_id) {
            container.status = status.to_string();
        }
        
        Ok(())
    }
}