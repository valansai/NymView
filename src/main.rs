use eframe::egui;
use clap::Parser;

use nymlib::nymsocket::SocketMode;


mod config;
mod mixnet_browser;


#[derive(Parser)]
#[command(name = "nym-view-browser")]
#[command(about = "NymView Browser - Explore pages on the Nym Mixnet")]
struct Cli {
    #[arg(long, value_enum, default_value_t = SocketMode::Anonymous)]
    socket_mode: SocketMode,
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([700.0, 800.0])
            .with_min_inner_size([500.0, 600.0])
            .with_title("NymView"),
        ..Default::default()
    };

    let cli = Cli::parse();

    eframe::run_native(
        "NymView",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(mixnet_browser::NymMixnetBrowser::new(cli.socket_mode)))
        }),
    )
}
