use nymlib::nymsocket::{Socket, SockAddr, SocketMode};
use nymlib::serialize::{DataStream, Serialize};
use egui::{Ui, TextEdit, ScrollArea, Color32};
use tokio::sync::mpsc;
use once_cell::sync::Lazy;
use tokio::runtime::Runtime;
use std::sync::{Arc, Mutex};
use std::sync::OnceLock;
use std::io::Write;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use eframe::App;
use rand::Rng;

use crate::config;


// Global runtime
static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().expect("Failed to create Tokio runtime")
});

// Global sender for requests
static GUI_TO_MIXNET_SENDER: OnceLock<Arc<Mutex<Option<mpsc::UnboundedSender<BrowserMessage>>>>> =
    OnceLock::new();

#[derive(Debug)]
pub(crate) enum BrowserMessage {
    SendRequest { recipient: String, message: DataStream },
    ReceivedMessage { content: String, from: String },
    ConnectionStatus { status: String, loading: bool, client_address: String },
}

#[derive(Debug, Clone)]
pub(crate) struct HistoryEntry {
    server: String,
    page: String,
}


pub struct NymMixnetBrowser {
    pub address_bar: String,
    pub current_content: String,
    pub loading: bool,
    pub page_loading: bool,
    pub error: Option<String>,
    pub connection_status: String,
    pub server_address: String,
    pub client_address: String,
    pub socket_mode: SocketMode,
    pub(crate) message_receiver: Option<mpsc::UnboundedReceiver<BrowserMessage>>,
    pub(crate) message_sender: Option<mpsc::UnboundedSender<BrowserMessage>>,
    pub(crate) history: Vec<HistoryEntry>,
    pub(crate) connection_attempted: bool,
    pub(crate) md_cache: CommonMarkCache,
    pub(crate) pending_navigation: Option<String>,
    pub(crate) pending_request_id: Arc<Mutex<Option<Vec<u8>>>>,
}

impl NymMixnetBrowser {
    pub fn new(socket_mode: SocketMode) -> Self {
        Self {
            address_bar: String::new(),
            current_content: String::new(),
            loading: true,
            page_loading: false,
            error: None,
            connection_status: "Connecting to Mixnet...".to_string(),
            server_address: String::new(),
            client_address: String::new(),
            socket_mode: socket_mode,
            message_receiver: None,
            message_sender: None,
            history: Vec::new(),
            connection_attempted: false,
            md_cache: CommonMarkCache::default(),
            pending_navigation: None,
            pending_request_id: Arc::new(Mutex::new(None)),
        }
    }

    pub fn init(&mut self) {
        if !self.connection_attempted {
            let (tx, rx) = mpsc::unbounded_channel::<BrowserMessage>();
            self.message_sender = Some(tx);
            self.message_receiver = Some(rx);
            self.connection_attempted = true;
            self.start_connection();
        }
    }

    fn start_connection(&mut self) {
        if let Some(sender) = self.message_sender.clone() {
            let browser = self.clone();
            RUNTIME.spawn(async move {
                match browser.connect_with_status(sender).await {
                    Ok(_) => println!("Connection successful"),
                    Err(e) => eprintln!("Connection failed: {}", e),
                }
            });
        }
    }

    async fn connect_with_status(&self, sender: mpsc::UnboundedSender<BrowserMessage>) -> Result<(), String> {

        let _ = sender.send(BrowserMessage::ConnectionStatus {
            status: "Connecting to Mixnet...".to_string(),
            loading: true,
            client_address: String::new(),
        });

        println!("Creating Mixnet Client...");
        
        let socket = Socket::new_ephemeral(self.socket_mode)
            .await
            .ok_or("Unable to build mixnet client")?;

        println!("Client created, connecting to Mixnet...");

        let mut listener = socket.clone();
        tokio::spawn(async move {
            listener.listen().await;
        });

        let client_address = socket.getsockaddr().await.unwrap().to_string();
        println!("Connected!");

        let _ = sender.send(BrowserMessage::ConnectionStatus {
            status: "Connected".to_string(),
            loading: false,
            client_address: client_address.clone(),
        });

        GUI_TO_MIXNET_SENDER.get_or_init(|| Arc::new(Mutex::new(None)));
        let (gui_to_mixnet_tx, gui_to_mixnet_rx) = mpsc::unbounded_channel::<BrowserMessage>();
        *GUI_TO_MIXNET_SENDER.get().unwrap().lock().unwrap() = Some(gui_to_mixnet_tx);

        RUNTIME.spawn(Self::mixnet_task(
            socket,
            gui_to_mixnet_rx,
            sender,
            self.pending_request_id.clone()
        ));

        Ok(())
    }

    
    async fn mixnet_task(
        socket: Socket,
        mut from_gui: mpsc::UnboundedReceiver<BrowserMessage>,
        to_gui: mpsc::UnboundedSender<BrowserMessage>,
        pending_request_id: Arc<Mutex<Option<Vec<u8>>>>,
    ) {
        let socket = Arc::new(tokio::sync::Mutex::new(socket));
        let socket_for_recv = socket.clone();

        loop {
            tokio::select! {
                // Handle incoming messages from the Mixnet receive queue
                messages = async {
                    let messages: Vec<_> = {
                        let mut guard = socket_for_recv.lock().await;
                        let mut recv = guard.recv.lock().await;
                        recv.drain(..).collect()
                    };
                    messages
                } => {
                    if !messages.is_empty() {
                        // Process each received message
                        for msg in messages {
                            let mut stream = DataStream::default();
                            let _ = stream.write(&msg.data);

                            // Read the command from the stream
                            let command = match stream.stream_out::<String>() {
                                Ok(cmd) => cmd,
                                Err(_) => {
                                    continue;
                                }
                            };

                            match command.as_str() {
                                // Handle the GET command (response to a requseted page)
                                config::COMMANDS::GET => {

                                    // Read the request ID from the stream
                                    let request_id = match stream.stream_out::<Vec<u8>>() {
                                        Ok(id) => id,
                                        Err(_) => continue,  
                                    };

                                    // Read the response from the stream
                                    let content = match stream.stream_out::<String>() {
                                        Ok(content) => content,
                                        Err(_) => continue,  
                                    };

                                    // Determine if the response matches the current pending request
                                    let accept = {
                                        let pending_id = pending_request_id.lock().unwrap().clone();
                                        if let Some(pending) = pending_id {
                                            pending == request_id
                                        } else {
                                            println!("Received content for unknown request_id {:?}", request_id);
                                            false
                                        }
                                    };


                                    if accept {
                                        // Forward the content to the GUI if the request ID matches
                                        let _ = to_gui.send(BrowserMessage::ReceivedMessage {
                                            content,
                                            from: msg.from.to_string(),
                                        });


                                        // Clear the pending request ID after processing
                                        *pending_request_id.lock().unwrap() = None;
                                    }
                                }

                                _ => {
                                    println!("Unknown command received: {}", command);
                                }
                            }        
                        }
                    }
                }
                
                // Handle messages sent from the GUI
                Some(gui_message) = from_gui.recv() => {
                    if let BrowserMessage::SendRequest { recipient, message } = gui_message {
                        // Parse the recipient address into a SockAddr
                        let sock_addr = SockAddr::from(recipient.as_str());

                        // Validation check 
                        if sock_addr.is_null() {
                            let _ = to_gui.send(BrowserMessage::ReceivedMessage {
                                content: "ERROR: Invalid address".to_string(),
                                from: "system".to_string(),
                            });
                            continue;
                        }

                        // Sent the request
                        let sent = {
                            let mut s = socket.lock().await;
                            s.extra_surbs = Some(5); // TODO
                            s.send(message.data, sock_addr).await
                        };

                        if sent {
                            println!("Sent request to {recipient}");
                        } else {
                            // 
                            let _ = to_gui.send(BrowserMessage::ReceivedMessage {
                                content: "ERROR: Failed to send (mixnet down?)".to_string(),
                                from: "local".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }


    fn get_gui_sender() -> Option<mpsc::UnboundedSender<BrowserMessage>> {
        GUI_TO_MIXNET_SENDER
            .get()
            .and_then(|arc| arc.lock().unwrap().clone())
    }

    pub fn send_request(&mut self, request_path: &str) -> Result<(), String> {
        // Read the server address
        let recipient = self.server_address.trim();
        if recipient.is_empty() {
            return Err("No server address specified".to_string());
        }


        // Create an unique request id 
        let request_id: Vec<u8> = (0..16).map(|_| rand::thread_rng().gen()).collect();

        // Create a new DataStream for the request
        let mut request_stream = DataStream::default();          
        let _ = request_stream.stream_in(&config::COMMANDS::ASK); // command
        let _ = request_stream.stream_in(&request_id);            // request ID
        let _ = request_stream.stream_in(&request_path);          // page path

        // Send the request via the GUI sender
        if let Some(sender) = Self::get_gui_sender() {
            sender.send(BrowserMessage::SendRequest {
                recipient: recipient.to_string(),
                message: request_stream,
            }).map_err(|e| format!("Send error: {}", e))?;
        } else {
            return Err("Not connected to Mixnet".to_string());
        }

        *self.pending_request_id.lock().unwrap() = Some(request_id.clone());

        Ok(())
    }

    fn parse_and_set_url(&mut self, url: &str) {
        if let Some((server, page)) = Self::parse_nym_url(url) {
            self.server_address = server.trim().to_string();
            self.address_bar = if page.is_empty() { String::new() } else { page };
        } else {
            self.address_bar = url.trim().to_string();
        }
    }

    fn parse_nym_url(url: &str) -> Option<(String, String)> {
        if !url.starts_with("nym://") {
            return None;
        }
        let without_protocol = &url[6..];
        if let Some(slash_pos) = without_protocol.find('/') {
            let server = without_protocol[..slash_pos].to_string();
            let page = without_protocol.get(slash_pos + 1..).unwrap_or("").to_string();
            Some((server, page))
        } else {
            Some((without_protocol.to_string(), "".to_string()))
        }
    }

    fn handle_navigation(&mut self) {
        let address = self.address_bar.clone();
        self.parse_and_set_url(&address);
        self.page_loading = true;

        let path = if self.address_bar.is_empty() {
            "index".to_string()
        } else if self.address_bar.starts_with('/') {
            self.address_bar.clone()
        } else {
            format!("{}", self.address_bar)
        };

        match self.send_request(&path) {
            Ok(()) => {},
            Err(e) => {
                self.error = Some(e);
                self.page_loading = false;
            }
        }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        ui.style_mut().url_in_tooltip = true;

        if !self.connection_attempted {
            self.init();
        }

        // Process pending navigation first
        if let Some(url) = self.pending_navigation.take() {
            self.handle_link_click(&url);
        }

        let mut messages_to_process = Vec::new();
        if let Some(receiver) = &mut self.message_receiver {
            while let Ok(message) = receiver.try_recv() {
                messages_to_process.push(message);
            }
        }

        for message in messages_to_process {
            match message {
                BrowserMessage::ReceivedMessage { content, from } => {
                    self.handle_server_message(content, from);
                }
                BrowserMessage::ConnectionStatus { status, loading, client_address } => {
                    self.connection_status = status;
                    self.loading = loading;
                    if !client_address.is_empty() {
                        self.client_address = client_address;
                    }
                }
                _ => {}
            }
        }

    
        // Address bar with responsive design
        ui.horizontal(|ui| {
            if ui.button("←").clicked() && self.history.len() > 1 {
                self.go_back();
            }
            
            ui.label("Address:");
            
            // Longer text field that adapts
            let text_width = ui.available_width() - 120.0;
            let response = ui.add(
                TextEdit::singleline(&mut self.address_bar)
                    .hint_text("nym://server/page")
                    .desired_width(text_width)
                    .min_size(egui::Vec2::new(550.0, 0.0))
            );

            let can_navigate = !self.loading && !self.address_bar.trim().is_empty();
            
            // Align buttons to the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Go").clicked() && can_navigate {
                    self.handle_navigation();
                }
                
                if self.page_loading {
                    ui.spinner();
                }
            });

            // Enter key handling
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && can_navigate {
                self.handle_navigation();
            }
        });

        if let Some(ref err) = self.error {
            ui.colored_label(Color32::BLUE, err);
        }

        ScrollArea::vertical().show(ui, |ui| {
            if self.page_loading {
                ui.vertical_centered(|ui| {
                    ui.spinner();
                    ui.colored_label(Color32::BLUE, "Loading via Mixnet...");
                });
            } else if self.current_content.is_empty() {
                self.show_welcome_page(ui);
            } else {
                // FIXED: Better link handling
                // First replace all nym:// links in content so they can't be clicked
                let safe_content = Self::replace_nym_links(&self.current_content);
                
                // Then render markdown with "safe" links
                CommonMarkViewer::new()
                    .show(ui, &mut self.md_cache, &safe_content);
                
                // MANUAL LINK DETECTION: Check for clicks in the entire content area
                let response = ui.allocate_rect(ui.max_rect(), egui::Sense::click());
                
                if response.clicked() {
                    let nym_links = Self::extract_nym_links(&self.current_content);
                    if !nym_links.is_empty() {
                        self.pending_navigation = Some(nym_links[0].clone());
                    }
                }
            }
        });

        egui::TopBottomPanel::bottom("footer_panel").show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Status:");
                if self.loading {
                    ui.spinner();
                    ui.colored_label(Color32::BLUE, "Connecting...");
                }
                else{
                    ui.colored_label(Color32::BLUE, &self.connection_status);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mode_label = match self.socket_mode {
                        SocketMode::Individual => "Pseudonymous".to_string(),
                        _ => format!("{:?}", self.socket_mode),
                    };
                    ui.label(format!("Mode: {}", mode_label));
                });
            });
        });
    }

    // NEW: Replace nym:// links with non-clickable text
    fn replace_nym_links(content: &str) -> String {
        let mut result = content.to_string();
        let links = Self::extract_nym_links(content);
        
        for link in links {
            // Replace nym:// links with plain text (without link formatting)
            let display_text = link.replace("nym://", "");
            result = result.replace(&link, &display_text);
        }
        
        result
    }

    // NEW: Improved link extraction
    fn extract_nym_links(content: &str) -> Vec<String> {
        let mut links = Vec::new();
        let mut search_pos = 0;
        
        while let Some(start) = content[search_pos..].find("nym://") {
            let actual_start = search_pos + start;
            let remaining = &content[actual_start..];
            
            // Find the end of the link
            let end = remaining.find(|c: char| c.is_whitespace() || c == ')' || c == ']' || c == '>' || c == '"' || c == '\'')
                .unwrap_or(remaining.len());
                
            let link = &remaining[..end];
            if !link.is_empty() && !links.contains(&link.to_string()) {
                links.push(link.to_string());
            }
            
            search_pos = actual_start + end;
            if search_pos >= content.len() {
                break;
            }
        }
        
        links
    }

    // NEW: handle_link_click method
    fn handle_link_click(&mut self, href: &str) {
        if href.starts_with("nym://") {
            // INTELLIGENT nym:// LINK PROCESSING
            let without_protocol = &href[6..]; // Remove "nym://"
            
            if without_protocol.contains('.') || without_protocol.len() > 50 {
                // FULL NYM ADDRESS (contains dots or is long)
                self.parse_and_set_url(href);
                self.page_loading = true;
                
                let path = if let Some((_, page)) = Self::parse_nym_url(href) {
                    if page.is_empty() { "/".to_string() } else { format!("/{}", page) }
                } else {
                    "/".to_string()
                };
                
                match self.send_request(&path) {
                    Ok(()) => {}, // Keine Ausgabe
                    Err(e) => {
                        self.error = Some(e);
                        self.page_loading = false;
                    }
                }
            } else {
                // RELATIVE PATH with nym:// prefix
                self.navigate_to(without_protocol);
            }
            
        } else if href.starts_with("http://") || href.starts_with("https://") {
            // EXTERNAL WEB LINKS
            self.error = Some("External web links not supported".to_string());
            
        } else if href.starts_with('/') {
            // ABSOLUTE PATHS
            let path = &href[1..]; // Remove leading slash
            self.navigate_to(path);
            
        } else {
            // RELATIVE PATHS (without prefix)
            self.navigate_to(href);
        }
    }

    fn navigate_to(&mut self, path: &str) {
        self.history.push(HistoryEntry {
            server: self.server_address.clone(),
            page: self.address_bar.clone(),
        });
        
        self.page_loading = true;
        
        let request_path = path.trim().trim_start_matches('/').to_string();

        match self.send_request(&request_path) {
            Ok(()) => {
                self.address_bar = path.to_string();
            }
            Err(e) => {
                self.error = Some(e);
                self.page_loading = false;
            }
        }
    }

    fn handle_server_message(&mut self, content: String, _from: String) {
        if content.starts_with("OK\n") {
            self.current_content = content[3..].to_string();
        } else {
            self.current_content = content;
        }
        self.error = None;
        self.page_loading = false;
    }

    fn go_back(&mut self) {
        if self.history.len() > 1 {
            if let Some(prev) = self.history.pop() {
                self.server_address = prev.server;
                self.address_bar = prev.page;
                self.page_loading = true;

                let path = if self.address_bar.is_empty() { 
                    "/".to_string() 
                } else if self.address_bar.starts_with('/') { 
                    self.address_bar.clone() 
                } else { 
                    format!("/{}", self.address_bar) 
                };
                
                match self.send_request(&path) {
                    Ok(()) => {},
                    Err(e) => {
                        self.error = Some(e);
                        self.page_loading = false;
                    }
                }
            }
        }
    }

    fn show_welcome_page(&self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.heading("NymView for Nym Mixnet");
            ui.label("Welcome! Enter a nym:// address to begin.");
            ui.separator();
            
            let demo_content = r#"# NymView for Nym Mixnet
            
## Features:
- **Secure** communication via Nym Mixnet
- **Markdown** support
- **Private** navigation

### Example content:
- `nym://server/` - Homepage
- `nym://server/about` - About us
- `nym://server/help` - Help

*Enter an address to begin*"#;
            
            CommonMarkViewer::new()
                .show(ui, &mut CommonMarkCache::default(), demo_content);
        });
    }
}

// App Trait Implementation for eframe
impl App for NymMixnetBrowser {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.show(ui);
        });
    }
}

impl Clone for NymMixnetBrowser {
    fn clone(&self) -> Self {
        Self {
            address_bar: self.address_bar.clone(),
            current_content: self.current_content.clone(),
            loading: self.loading,
            page_loading: self.page_loading,
            error: self.error.clone(),
            connection_status: self.connection_status.clone(),
            server_address: self.server_address.clone(),
            client_address: self.client_address.clone(),
            socket_mode: self.socket_mode,
            message_receiver: None,
            message_sender: None,
            history: self.history.clone(),
            connection_attempted: self.connection_attempted,
            md_cache: CommonMarkCache::default(),
            pending_navigation: None,
            pending_request_id: self.pending_request_id.clone(),
        }
    }
}


