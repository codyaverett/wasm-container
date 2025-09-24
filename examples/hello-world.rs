use std::env;

fn main() {
    println!("Hello from WASM Container!");
    
    println!("Environment variables:");
    for (key, value) in env::vars() {
        println!("  {}={}", key, value);
    }
    
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        println!("Command line arguments: {:?}", &args[1..]);
    }
    
    println!("Container execution completed successfully!");
}