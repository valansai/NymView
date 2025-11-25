use nymlib::nymsocket::{Socket, SockAddr, SocketMode};
use nymlib::serialize::{DataStream, Serialize};

use std::collections::HashMap;
use std::io::Write;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;
use std::sync::Arc;

use crate::config;
use crate::default_page;

pub struct NymMixnetServer {
    nym_socket: Socket,
    sites_dir: PathBuf,
    pub nym_address: String,
    cache: Arc<RwLock<HashMap<String, String>>>,
}

impl NymMixnetServer {
    pub async fn new(sites_directory: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // PERSISTENT CLIENT with configuration directory
        let config_dir = config::ensure_config_dir()?;
        let client_path = config_dir.join(sites_directory);
        let client_path_str = client_path.to_str()
            .ok_or("Config path contains invalid UTF-8")?;

        let socket = Socket::new_standard(client_path_str, SocketMode::Individual)
            .await
            .ok_or("No gateway reachable")?;

        let nym_address = socket.getsockaddr()
            .await.unwrap()
            .to_string();
        

        let sites_dir = PathBuf::from(sites_directory);
        if !sites_dir.exists() {
            fs::create_dir_all(&sites_dir)?;
            println!("Pages directory created: {:?}", sites_dir);
        }
        
        let cache = Self::load_sites_into_cache(&sites_dir).await?;
        
        println!("NymView Server started: nym://{}", nym_address);
        println!("Hosting from: {:?}", sites_dir);
        
        Ok(Self {
            nym_socket: socket,
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


        // Listening for incoming messages
        let mut listener = self.nym_socket.clone();
        tokio::spawn(async move {
            listener.listen().await;
        });

        println!("Server listening...");

        let socket = Arc::new(tokio::sync::Mutex::new(&mut self.nym_socket));
        
        loop {


            // Collect all pending messages from the receive queue
            let messages: Vec<_> = {
                let mut guard = socket.lock().await;
                let mut recv = guard.recv.lock().await;
                recv.drain(..).collect()
            };

            // Process each received message
            for msg in messages {
                let mut stream = DataStream::default();
                let _ = stream.write(&msg.data);


                // Read the command from the stream
                let command = match stream.stream_out::<String>() {
                    Ok(cmd) => cmd,
                    Err(_) => {
                        println!("Invalid message format: missing command");
                        continue;
                    }
                };

                match command.as_str() {
                    // Handle the ASK command (request for a page)
                    config::COMMANDS::ASK => {


                        // Read the request ID from the stream
                        let request_id = match stream.stream_out::<Vec<u8>>() {
                            Ok(id) => id,
                            Err(_) => continue,  
                        };


                        // Read the requested page path from the stream
                        let request_page = match stream.stream_out::<String>() {
                            Ok(page) => page,
                            Err(_) => continue,  
                        };


                        // Read the current cached pages
                        let cache = self.cache.read().await;
                        
                        // Look up the requested page in the cache: if it's not found, use the default 404 page
                        let response = match cache.get(&request_page) {
                            Some(content) => content.clone(),
                            None => default_page::default_404().to_string(),
                        };

                        // Create a new DataStream for the response
                        let mut response_stream = DataStream::default();
                        let _ = response_stream.stream_in(&config::COMMANDS::GET); // command
                        let _ = response_stream.stream_in(&request_id);            // request ID
                        let _ = response_stream.stream_in(&response);              // response

                        // Send the response back to the sender
                        let sent = {
                            let mut s = socket.lock().await; 
                            s.send(response_stream.data, msg.from.clone()).await
                        };

                        println!("{:?} to {:?}", sent, msg.from.clone().to_string());
                    }

                    _ => {
                        println!("Unknown command received: {}", command);
                    }
                }        
            }
            
            
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
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