use anyhow::Result;
use std::path::{Path, PathBuf};
use std::fs;
use tempfile::TempDir;
use tar::Archive;
use flate2::read::GzDecoder;
use tracing::{info, debug};

use crate::container::Container;

pub struct Filesystem {
    container_id: String,
    rootfs: TempDir,
    layers: Vec<PathBuf>,
}

impl Filesystem {
    pub fn new(container: &Container) -> Result<Self> {
        let rootfs = TempDir::new()?;
        
        Ok(Self {
            container_id: container.id().to_string(),
            rootfs,
            layers: Vec::new(),
        })
    }
    
    pub async fn setup(&self) -> Result<()> {
        info!("Setting up filesystem for container: {}", self.container_id);
        
        self.create_base_directories()?;
        self.mount_proc_sys()?;
        self.setup_resolv_conf()?;
        
        Ok(())
    }
    
    pub fn rootfs_path(&self) -> &Path {
        self.rootfs.path()
    }
    
    fn create_base_directories(&self) -> Result<()> {
        let dirs = [
            "bin", "boot", "dev", "etc", "home", "lib", "lib64",
            "media", "mnt", "opt", "proc", "root", "run", "sbin",
            "srv", "sys", "tmp", "usr", "var",
        ];
        
        for dir in &dirs {
            let path = self.rootfs.path().join(dir);
            fs::create_dir_all(&path)?;
        }
        
        let usr_dirs = ["bin", "sbin", "lib", "lib64", "local", "share", "include"];
        for dir in &usr_dirs {
            let path = self.rootfs.path().join("usr").join(dir);
            fs::create_dir_all(&path)?;
        }
        
        let var_dirs = ["log", "cache", "lib", "run", "tmp"];
        for dir in &var_dirs {
            let path = self.rootfs.path().join("var").join(dir);
            fs::create_dir_all(&path)?;
        }
        
        Ok(())
    }
    
    fn mount_proc_sys(&self) -> Result<()> {
        fs::write(
            self.rootfs.path().join("proc").join("cpuinfo"),
            "processor\t: 0\nvendor_id\t: WASM\nmodel name\t: WASM Container Runtime\n",
        )?;
        
        fs::write(
            self.rootfs.path().join("proc").join("meminfo"),
            "MemTotal:        8388608 kB\nMemFree:         4194304 kB\n",
        )?;
        
        Ok(())
    }
    
    fn setup_resolv_conf(&self) -> Result<()> {
        fs::write(
            self.rootfs.path().join("etc").join("resolv.conf"),
            "nameserver 8.8.8.8\nnameserver 8.8.4.4\n",
        )?;
        
        fs::write(
            self.rootfs.path().join("etc").join("hostname"),
            &self.container_id,
        )?;
        
        fs::write(
            self.rootfs.path().join("etc").join("hosts"),
            format!("127.0.0.1\tlocalhost\n127.0.1.1\t{}\n", self.container_id),
        )?;
        
        Ok(())
    }
    
    pub async fn extract_layer(&mut self, layer_path: &Path) -> Result<()> {
        debug!("Extracting layer: {:?}", layer_path);
        
        let tar_gz = fs::File::open(layer_path)?;
        let tar = GzDecoder::new(tar_gz);
        let mut archive = Archive::new(tar);
        
        archive.unpack(self.rootfs.path())?;
        
        self.layers.push(layer_path.to_path_buf());
        
        Ok(())
    }
    
    pub fn create_device_nodes(&self) -> Result<()> {
        let devices = [
            ("null", 1, 3),
            ("zero", 1, 5),
            ("random", 1, 8),
            ("urandom", 1, 9),
            ("tty", 5, 0),
            ("console", 5, 1),
        ];
        
        for (name, _major, _minor) in &devices {
            let path = self.rootfs.path().join("dev").join(name);
            fs::write(&path, "")?;
        }
        
        Ok(())
    }
    
    pub fn mount_volume(&self, host_path: &Path, container_path: &Path) -> Result<()> {
        let target = self.rootfs.path().join(
            container_path.strip_prefix("/").unwrap_or(container_path)
        );
        
        if host_path.is_dir() {
            fs::create_dir_all(&target)?;
            for entry in fs::read_dir(host_path)? {
                let entry = entry?;
                let file_name = entry.file_name();
                let src = entry.path();
                let dst = target.join(&file_name);
                
                if src.is_dir() {
                    self.copy_dir_recursive(&src, &dst)?;
                } else {
                    fs::copy(&src, &dst)?;
                }
            }
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(host_path, &target)?;
        }
        
        Ok(())
    }
    
    fn copy_dir_recursive(&self, src: &Path, dst: &Path) -> Result<()> {
        fs::create_dir_all(dst)?;
        
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let src_path = entry.path();
            let dst_path = dst.join(&file_name);
            
            if src_path.is_dir() {
                self.copy_dir_recursive(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }
        
        Ok(())
    }
}