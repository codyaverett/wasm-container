use anyhow::Result;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::Mutex;
use std::sync::Arc;
use tracing::{info, debug, error};

use crate::container::Container;

pub struct NetworkManager {
    networks: Arc<Mutex<HashMap<String, Network>>>,
    port_forwards: Arc<Mutex<HashMap<u16, PortForward>>>,
}

#[derive(Debug, Clone)]
pub struct Network {
    pub name: String,
    pub subnet: String,
    pub gateway: IpAddr,
    pub containers: Vec<String>,
}

#[derive(Debug)]
pub struct PortForward {
    pub host_port: u16,
    pub container_id: String,
    pub container_port: u16,
    pub protocol: String,
    pub listener: Option<TcpListener>,
}

impl NetworkManager {
    pub fn new() -> Self {
        let mut networks = HashMap::new();
        
        networks.insert(
            "bridge".to_string(),
            Network {
                name: "bridge".to_string(),
                subnet: "172.17.0.0/16".to_string(),
                gateway: IpAddr::V4(Ipv4Addr::new(172, 17, 0, 1)),
                containers: Vec::new(),
            }
        );
        
        Self {
            networks: Arc::new(Mutex::new(networks)),
            port_forwards: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    pub async fn setup_container_network(&self, container: &Container) -> Result<ContainerNetwork> {
        debug!("Setting up network for container: {}", container.id());
        
        let ip = self.allocate_ip(container.id()).await?;
        
        let mut port_mappings = Vec::new();
        for port_map in &container.network_config().ports {
            self.setup_port_forward(
                container.id(),
                port_map.host_port,
                port_map.container_port,
                &port_map.protocol,
            ).await?;
            
            port_mappings.push((*port_map).clone());
        }
        
        Ok(ContainerNetwork {
            container_id: container.id().to_string(),
            ip_address: ip,
            hostname: container.network_config().hostname.clone(),
            port_mappings,
        })
    }
    
    pub async fn cleanup_container_network(&self, container_id: &str) -> Result<()> {
        info!("Cleaning up network for container: {}", container_id);
        
        let mut port_forwards = self.port_forwards.lock().await;
        let forwards_to_remove: Vec<u16> = port_forwards
            .iter()
            .filter(|(_, forward)| forward.container_id == container_id)
            .map(|(&port, _)| port)
            .collect();
        
        for port in forwards_to_remove {
            port_forwards.remove(&port);
            debug!("Removed port forward for port: {}", port);
        }
        
        let mut networks = self.networks.lock().await;
        for network in networks.values_mut() {
            network.containers.retain(|id| id != container_id);
        }
        
        Ok(())
    }
    
    async fn allocate_ip(&self, container_id: &str) -> Result<IpAddr> {
        let mut networks = self.networks.lock().await;
        
        if let Some(bridge_network) = networks.get_mut("bridge") {
            let container_count = bridge_network.containers.len();
            let ip = IpAddr::V4(Ipv4Addr::new(172, 17, 0, (container_count + 2) as u8));
            
            bridge_network.containers.push(container_id.to_string());
            
            Ok(ip)
        } else {
            Ok(IpAddr::V4(Ipv4Addr::new(172, 17, 0, 2)))
        }
    }
    
    async fn setup_port_forward(
        &self,
        container_id: &str,
        host_port: u16,
        container_port: u16,
        protocol: &str,
    ) -> Result<()> {
        debug!(
            "Setting up port forward: {}:{} -> {}:{}",
            host_port, protocol, container_id, container_port
        );
        
        match protocol.to_lowercase().as_str() {
            "tcp" => {
                let listener = TcpListener::bind(SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
                    host_port,
                )).await?;
                
                let port_forward = PortForward {
                    host_port,
                    container_id: container_id.to_string(),
                    container_port,
                    protocol: protocol.to_string(),
                    listener: Some(listener),
                };
                
                self.port_forwards.lock().await.insert(host_port, port_forward);
                
                info!("TCP port forward established: {} -> {}", host_port, container_port);
            }
            "udp" => {
                let _socket = UdpSocket::bind(SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
                    host_port,
                )).await?;
                
                let port_forward = PortForward {
                    host_port,
                    container_id: container_id.to_string(),
                    container_port,
                    protocol: protocol.to_string(),
                    listener: None,
                };
                
                self.port_forwards.lock().await.insert(host_port, port_forward);
                
                info!("UDP port forward established: {} -> {}", host_port, container_port);
            }
            _ => {
                error!("Unsupported protocol: {}", protocol);
            }
        }
        
        Ok(())
    }
    
    pub async fn create_network(&self, name: &str, subnet: &str) -> Result<()> {
        let mut networks = self.networks.lock().await;
        
        if networks.contains_key(name) {
            return Err(anyhow::anyhow!("Network {} already exists", name));
        }
        
        let network = Network {
            name: name.to_string(),
            subnet: subnet.to_string(),
            gateway: IpAddr::V4(Ipv4Addr::new(172, 18, 0, 1)),
            containers: Vec::new(),
        };
        
        networks.insert(name.to_string(), network);
        
        info!("Created network: {} with subnet: {}", name, subnet);
        
        Ok(())
    }
    
    pub async fn list_networks(&self) -> Result<Vec<Network>> {
        let networks = self.networks.lock().await;
        Ok(networks.values().cloned().collect())
    }
    
    pub async fn get_container_ip(&self, container_id: &str) -> Result<Option<IpAddr>> {
        let networks = self.networks.lock().await;
        
        for network in networks.values() {
            if let Some(index) = network.containers.iter().position(|id| id == container_id) {
                let ip = match network.name.as_str() {
                    "bridge" => IpAddr::V4(Ipv4Addr::new(172, 17, 0, (index + 2) as u8)),
                    _ => IpAddr::V4(Ipv4Addr::new(172, 18, 0, (index + 2) as u8)),
                };
                return Ok(Some(ip));
            }
        }
        
        Ok(None)
    }
}

#[derive(Debug)]
pub struct ContainerNetwork {
    pub container_id: String,
    pub ip_address: IpAddr,
    pub hostname: String,
    pub port_mappings: Vec<crate::container::PortMapping>,
}

impl ContainerNetwork {
    pub fn get_ip(&self) -> IpAddr {
        self.ip_address
    }
    
    pub fn get_hostname(&self) -> &str {
        &self.hostname
    }
}