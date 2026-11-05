//! The bikestudio app (S-SHELL).

use bike_application::ui::{Studio, apply_theme, bind_keys};
use gpui_kit::component::{Root, Theme, ThemeMode};
use gpui_kit::*;

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            apply_theme(cx);
            bind_keys(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1600.0), px(1000.0)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("bikestudio".into()),
                    ..Default::default()
                }),
                app_id: Some("bikestudio".into()),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                let studio = cx.new(|cx| Studio::new(window, cx));
                cx.new(|cx| Root::new(studio, window, cx))
            })
            .expect("failed to open the bikestudio window");
            cx.activate(true);
        });
}
