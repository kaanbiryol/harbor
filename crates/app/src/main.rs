use gpui::{AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, Theme, ThemeMode, TitleBar};
use harbor_ui::{AppView, bind_keys};

fn main() {
    gpui_platform::application().run(move |cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        bind_keys(cx);

        let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitleBar::title_bar_options()),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| AppView::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("failed to open application window");
        })
        .detach();
    });
}
