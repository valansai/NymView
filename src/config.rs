use std::path::PathBuf;

pub fn get_config_dir() -> PathBuf {
    let mut config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config_dir.push("NymView");
    config_dir.push("mixnet_server");
    config_dir
}

pub fn ensure_config_dir() -> std::io::Result<PathBuf> {
    let config_dir = get_config_dir();
    std::fs::create_dir_all(&config_dir)?;
    println!("Persistence directory: {:?}", config_dir);
    Ok(config_dir)
}


pub mod COMMANDS {
    pub const ASK: &str = "ASK";
    pub const GET: &str = "GET";   
}