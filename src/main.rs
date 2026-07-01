use color_eyre::eyre;

use crate::{ emu::emulator::Emulator, interface::init };


#[cfg(not(target_arch = "wasm32"))]
mod cli;
#[cfg(not(target_arch = "wasm32"))]
mod utils;
#[cfg(not(target_arch = "wasm32"))]
mod filesystem;

mod emu;
mod shaders;
mod interface;


#[cfg(not(target_arch = "wasm32"))]
fn main() -> eyre::Result<()> {
    use std::fs;
    use clap::Parser;
    use crate::{
        utils::Converter,
        cli::{Cli, MainArgs},
        interface::title::MAIN_WINDOW_TITLE
    };

    // Init runtime stuff
    color_eyre::install()?;
    // init_logger();

    // Parse arguments
    let args = Cli::parse();
    let mode = args.mode.unwrap_or(cli::Command::Main(MainArgs::default()));
    match mode {
        cli::Command::Main(_) => {
            
            eframe::run_native(
                MAIN_WINDOW_TITLE,
                eframe::NativeOptions::default(),
                Box::new(|cc| {
                    let emu = initialize(cc)?;
                    Ok(Box::new(emu))
                }
            ))?;
            
        },
        cli::Command::Utils(util_args) => match util_args.util {

            cli::UtilCommand::Converter(converter_args) => {

                for path in converter_args.files {
                    if path.is_file() {
                        let converted = Converter::convert(&path)?;
                        let mut p = path.clone(); p.add_extension("bin");
                        fs::write(p, &converted)?;
                    }
                }

            }

        }
    }

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    wasm_bindgen_futures::spawn_local(async {
        // Get the document
        let document = web_sys::window()
            .expect("No window").document().expect("No document");

        // Get the canvas
        let canvas = document.get_element_by_id("game_canvas")
            .expect("Failed to find game_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("game_canvas was not a HtmlCanvasElement");

        // Let's go!
        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    let emu = initialize(cc)?;
                    Ok(Box::new(emu))
                }),
            )
            .await;

        // Remove the loading text and spinner
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => loading_text.remove(),
                Err(e) => {
                    loading_text.set_inner_html("<p> The app has crashed. See the developer console for details. </p>");
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}

fn initialize(cc: &eframe::CreationContext<'_>) -> eyre::Result<Emulator> {
    // Init egui stuff
    init::load_fonts(&cc.egui_ctx);
    let texture = init::create_default_texture(&cc.egui_ctx);
    // Init the emulator
    Emulator::new(cc, texture)
}

// fn init_logger() {
//     let file_appender = rolling::never(".", "emulator.log");

//     tracing_subscriber::fmt()
//         .with_writer(file_appender)
//         .without_time()
//         .with_target(false)
//         .with_level(false)
//         .init();
// }
