mod assets;
mod onboarding;
mod settings;
mod theme;
mod window_chrome;

use gpui::{
    App, Bounds, QuitMode, SharedString, TitlebarOptions, WindowBounds, WindowOptions, point,
    prelude::*, px, size,
};
use gpui_platform::application;

use crate::onboarding::Onboarding;

fn main() {
    application()
        .with_assets(assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx: &mut App| {
            let window_size = size(
                px(onboarding::WINDOW_WIDTH),
                px(onboarding::WINDOW_HEIGHT),
            );
            let bounds = Bounds::centered(None, window_size, cx);

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from("VLC")),
                        // We draw our own (blank) title strip so the onboarding
                        // matches the design on both Windows and macOS.
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(12.), px(12.))),
                    }),
                    is_resizable: false,
                    window_min_size: Some(window_size),
                    app_id: Some("org.redesignedvlc.app".into()),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Onboarding::new(window, cx)),
            )
            .expect("failed to open onboarding window");

            cx.activate(true);
        });
}
