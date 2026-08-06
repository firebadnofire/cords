mod app;
mod model;
mod theme;
mod widgets;

use app::CordsApp;
use eframe::egui;

fn main() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Cords")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([960.0, 640.0])
        .with_icon(load_icon());

    eframe::run_native(
        "Cords",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(CordsApp::new(cc)))),
    )
}

fn load_icon() -> egui::IconData {
    const SIZE: usize = 64;
    let mut rgba = vec![0_u8; SIZE * SIZE * 4];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let i = (y * SIZE + x) * 4;
            let dx = x as f32 - 31.5;
            let dy = y as f32 - 31.5;
            let inside = dx * dx + dy * dy < 29.0 * 29.0;
            let bubble = inside && !(y > 48 && x < 23);
            let tail = y >= 41 && y <= 55 && x >= 10 && x <= 25 && x + y >= 59;
            let white = (y >= 28 && y <= 35)
                && ((x >= 18 && x <= 25) || (x >= 31 && x <= 38) || (x >= 44 && x <= 51));
            let color = if white {
                [244, 246, 255, 255]
            } else if bubble || tail {
                [112, 132, 255, 255]
            } else {
                [0, 0, 0, 0]
            };
            rgba[i..i + 4].copy_from_slice(&color);
        }
    }
    egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
}
