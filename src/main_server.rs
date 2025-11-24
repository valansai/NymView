use clap::Parser;

mod default_page;
mod mixnet_server;
mod config;

#[derive(Parser)]
#[command(name = "nym-view-server")]
#[command(about = "NymView Server - Host MarkDown pages on the Nym Mixnet")]
struct Cli {
    #[arg(short, long, default_value = "./pages")]
    directory: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut server = mixnet_server::NymMixnetServer::new(&cli.directory).await?;
    println!("Server address: {}", server.get_nym_address());
    server.start().await?;
    Ok(())
}
