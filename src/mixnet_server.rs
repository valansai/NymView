use nym_sdk::mixnet;
use nym_sdk::mixnet::MixnetMessageSender;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;
use std::sync::Arc;
use crate::config;
use crate::default_page;

pub struct NymMixnetServer {
    nym_client: mixnet::MixnetClient,
    sites_dir: PathBuf,
    pub nym_address: String,
    cache: Arc<RwLock<HashMap<String, String>>>,
}

impl NymMixnetServer {
    pub async fn new(sites_directory: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // PERSISTENT CLIENT with configuration directory
        let config_dir = config::ensure_config_dir()?;
        let client_path = config_dir.join("mixnet_client");
        
        println!("Persistent client path: {:?}", client_path);
        
        // FIXED: Final API with type conversion
        let storage_paths = nym_sdk::mixnet::StoragePaths::new_from_dir(&client_path)?;
        let storage = nym_sdk::mixnet::OnDiskPersistent::from_paths(
            storage_paths.into(), // .into() for type conversion
            &Default::default(),
        ).await?;
        
        let client = mixnet::MixnetClientBuilder::new_with_storage(storage)
            .build()?;
        
        let connected_client = client.connect_to_mixnet().await?;
        let nym_address = connected_client.nym_address().to_string();
        
        let sites_dir = PathBuf::from(sites_directory);
        if !sites_dir.exists() {
            fs::create_dir_all(&sites_dir)?;
            println!("Pages directory created: {:?}", sites_dir);
        }
        
        let cache = Self::load_sites_into_cache(&sites_dir).await?;
        
        println!("NymView Server started: {}", nym_address);
        println!("Hosting from: {:?}", sites_dir);
        println!("Persistent keys saved in: {:?}", config_dir);
        
        Ok(Self {
            nym_client: connected_client,
            sites_dir,
            nym_address,
            cache: Arc::new(RwLock::new(cache)),
        })
    }
    
    async fn load_sites_into_cache(sites_dir: &Path) -> Result<HashMap<String, String>, std::io::Error> {
        let mut cache = HashMap::new();
        
        if let Ok(entries) = fs::read_dir(sites_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(extension) = path.extension() {
                        if extension == "md" || extension == "markdown" {
                            if let Ok(content) = fs::read_to_string(&path) {
                                if let Some(file_name) = path.file_stem().and_then(|s| s.to_str()) {
                                    cache.insert(file_name.to_string(), content);
                                    println!("Loaded: {} -> {}", file_name, path.display());
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Default homepage if no index.md exists
        if !cache.contains_key("index") {
            cache.insert("index".to_string(), default_page::default_index().to_string());
            println!("Serving default index page");
        }
        
        Ok(cache)
    }
    
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Server listening...");
        
        loop {
            if let Some(messages) = self.nym_client.wait_for_messages().await {
                for received in messages {
                    if let Ok(text_message) = String::from_utf8(received.message.clone()) {
                        let (response, reply_to) = self.handle_request(&text_message).await;
                        
                        if let Some(target) = reply_to {
                            match target.parse::<nym_sdk::mixnet::Recipient>() {
                                Ok(recipient) => {
                                    if let Err(e) = self.nym_client.send_plain_message(recipient, response).await {
                                        eprintln!("Error sending response: {}", e);
                                    }
                                    // Keine "Response sent successfully" Ausgabe mehr
                                }
                                Err(e) => {
                                    eprintln!("Invalid response address: {}", e);
                                }
                            }
                        } else {
                            eprintln!("No response address in request");
                        }
                    }
                }
            }
            
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    }
    
    async fn handle_request(&self, request: &str) -> (String, Option<String>) {
        // Search for " FROM " from the back (in case path contains spaces)
        if let Some(from_pos) = request.rfind(" FROM ") {
            let actual_request = &request[..from_pos];
            let client_address = request[from_pos + 6..].trim().to_string();
            
            let response = self.process_command(actual_request).await;
            (response, Some(client_address))
        } else {
            // Old requests or errors
            let response = "ERROR: Request must be 'GET /path FROM your_address'".to_string();
            (response, None)
        }
    }
    
    async fn process_command(&self, request: &str) -> String {
        let parts: Vec<&str> = request.splitn(2, ' ').collect();
        if parts.len() != 2 {
            return "ERROR: Invalid request format".to_string();
        }
        
        let command = parts[0];
        let path = parts[1].trim();
        
        match command {
            "GET" => self.serve_page(path).await,
            "LIST" => self.list_pages().await,
            "PING" => "PONG".to_string(),
            "RELOAD" => self.reload_cache().await,
            _ => format!("ERROR: Unknown command: {}", command),
        }
    }
    
    async fn serve_page(&self, path: &str) -> String {
        let clean_path = if path == "/" { "index" } else { path.trim_start_matches('/') };
        
        let cache = self.cache.read().await;
        match cache.get(clean_path) {
            Some(content) => {
                format!("OK\n{}", content)
            }
            None => {
                format!("ERROR: Page '{}' not found", clean_path)
            }
        }
    }
    
    async fn list_pages(&self) -> String {
        let cache = self.cache.read().await;
        let pages: Vec<&str> = cache.keys().map(|k| k.as_str()).collect();
        format!("OK\n{}", pages.join(","))
    }
    
    async fn reload_cache(&self) -> String {
        match Self::load_sites_into_cache(&self.sites_dir).await {
            Ok(new_cache) => {
                let mut cache = self.cache.write().await;
                *cache = new_cache;
                "OK\nCache reloaded".to_string()
            }
            Err(e) => format!("ERROR: Error reloading: {}", e),
        }
    }
    
    pub fn get_nym_address(&self) -> &str {
        &self.nym_address
    }
}
