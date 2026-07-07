mod app_icon;
mod assets;

use assets::Assets;
use gpui::{AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, Theme, ThemeMode, TitleBar};
use harbor_ui::{AppView, bind_keys};
use std::sync::Arc;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt};

fn main() {
    install_rustls_provider();
    init_logging();

    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx| {
            app_icon::install();
            let http_client = reqwest_client::ReqwestClient::user_agent("harbor")
                .expect("failed to initialize GPUI HTTP client");
            cx.set_http_client(Arc::new(http_client));
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            let theme = Theme::global_mut(cx);
            theme.radius = px(0.);
            theme.radius_lg = px(0.);
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

fn install_rustls_provider() {
    // Octocrab and GPUI's HTTP client can enable different Rustls providers
    // through feature unification, so choose the provider before either client
    // performs TLS setup.
    drop(rustls::crypto::aws_lc_rs::default_provider().install_default());
}

fn init_logging() {
    let filter = Targets::new()
        .with_default(tracing::Level::WARN)
        .with_target("harbor_github", tracing::Level::INFO)
        .with_target("harbor_ui", tracing::Level::INFO);

    drop(
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().with_target(true))
            .with(filter)
            .try_init(),
    );
}
