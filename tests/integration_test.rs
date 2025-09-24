use wasm_container::runtime::WasmRuntime;
use wasm_container::container::Container;
use wasm_container::image::{ImageData, ImageConfig, Layer};
use std::path::PathBuf;
use std::collections::HashMap;
use tokio_test;

#[tokio::test]
async fn test_basic_container_execution() {
    let image_data = create_test_image();
    let container = Container::new(image_data, None, None, vec![]).unwrap();
    
    let mut runtime = WasmRuntime::new().unwrap();
    
    let result = runtime.run(container).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_container_with_env_vars() {
    let image_data = create_test_image();
    let env_vars = vec!["TEST_VAR=test_value".to_string()];
    let container = Container::new(image_data, None, None, env_vars).unwrap();
    
    let mut runtime = WasmRuntime::new().unwrap();
    
    let result = runtime.run(container).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_container_listing() {
    let mut runtime = WasmRuntime::new().unwrap();
    
    let containers = runtime.list_containers(false).await.unwrap();
    assert_eq!(containers.len(), 0);
    
    let containers_all = runtime.list_containers(true).await.unwrap();
    assert_eq!(containers_all.len(), 0);
}

#[tokio::test]
async fn test_container_stop() {
    let mut runtime = WasmRuntime::new().unwrap();
    
    let result = runtime.stop("nonexistent-id").await;
    assert!(result.is_ok());
}

fn create_test_image() -> ImageData {
    ImageData {
        name: "test-image".to_string(),
        tag: "latest".to_string(),
        layers: vec![Layer {
            digest: "sha256:test".to_string(),
            size: 1024,
            media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
            path: PathBuf::from("/tmp/test-layer.tar.gz"),
        }],
        config: ImageConfig {
            env: vec!["PATH=/usr/bin".to_string()],
            cmd: vec!["/bin/sh".to_string()],
            entrypoint: vec![],
            workdir: "/".to_string(),
            exposed_ports: HashMap::new(),
            volumes: HashMap::new(),
        },
        wasm_path: Some(PathBuf::from("src/image/demo.wasm")),
    }
}