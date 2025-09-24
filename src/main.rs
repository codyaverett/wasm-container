use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{info, error};
use tracing_subscriber;

mod runtime;
mod container;
mod image;
mod filesystem;
mod network;

use crate::runtime::WasmRuntime;
use crate::container::Container;
use crate::image::ImageManager;

#[derive(Parser)]
#[command(name = "wasm-container")]
#[command(about = "A WASM container runtime that can run Docker containers", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(help = "Container image to run")]
        image: String,
        
        #[arg(short, long, help = "Command to execute in container")]
        command: Option<Vec<String>>,
        
        #[arg(short, long, help = "Working directory")]
        workdir: Option<String>,
        
        #[arg(short, long, help = "Environment variables")]
        env: Vec<String>,
    },
    
    Pull {
        #[arg(help = "Image to pull")]
        image: String,
    },
    
    List {
        #[arg(short, long, help = "List all containers including stopped")]
        all: bool,
    },
    
    Stop {
        #[arg(help = "Container ID to stop")]
        container_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Run { image, command, workdir, env } => {
            info!("Running container from image: {}", image);
            run_container(image, command, workdir, env).await?;
        }
        Commands::Pull { image } => {
            info!("Pulling image: {}", image);
            pull_image(image).await?;
        }
        Commands::List { all } => {
            list_containers(all).await?;
        }
        Commands::Stop { container_id } => {
            stop_container(container_id).await?;
        }
    }
    
    Ok(())
}

async fn run_container(
    image: String, 
    command: Option<Vec<String>>,
    workdir: Option<String>,
    env: Vec<String>
) -> Result<()> {
    let mut runtime = WasmRuntime::new()?;
    let image_manager = ImageManager::new()?;
    
    let image_data = image_manager.get_or_pull(&image).await?;
    
    let container = Container::new(image_data, command, workdir, env)?;
    
    runtime.run(container).await?;
    
    Ok(())
}

async fn pull_image(image: String) -> Result<()> {
    let image_manager = ImageManager::new()?;
    image_manager.pull(&image).await?;
    info!("Successfully pulled image: {}", image);
    Ok(())
}

async fn list_containers(all: bool) -> Result<()> {
    let runtime = WasmRuntime::new()?;
    let containers = runtime.list_containers(all).await?;
    
    println!("CONTAINER ID\tIMAGE\tSTATUS");
    for container in containers {
        println!("{}\t{}\t{}", container.id, container.image, container.status);
    }
    
    Ok(())
}

async fn stop_container(container_id: String) -> Result<()> {
    let mut runtime = WasmRuntime::new()?;
    runtime.stop(&container_id).await?;
    info!("Container {} stopped", container_id);
    Ok(())
}