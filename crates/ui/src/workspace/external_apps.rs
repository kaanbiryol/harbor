use std::collections::HashMap;

use gpui::{AppContext, Context};
use harbor_git::ExternalApp;

use crate::workspace::{AppView, async_updates::AppViewAsyncUpdateExt};

#[derive(Clone, Debug)]
pub(crate) struct ExternalAppAvailability {
    apps: HashMap<ExternalApp, bool>,
    is_loading: bool,
}

impl Default for ExternalAppAvailability {
    fn default() -> Self {
        Self {
            apps: ExternalApp::ALL
                .into_iter()
                .map(|app| (app, default_app_availability(app)))
                .collect(),
            is_loading: true,
        }
    }
}

impl ExternalAppAvailability {
    fn detect() -> Self {
        Self {
            apps: ExternalApp::ALL
                .into_iter()
                .map(|app| (app, app.is_available()))
                .collect(),
            is_loading: false,
        }
    }

    pub(crate) fn is_available(&self, app: ExternalApp) -> bool {
        self.apps.get(&app).copied().unwrap_or(false)
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.is_loading
    }
}

impl AppView {
    pub(crate) fn external_app_is_available(&self, app: ExternalApp) -> bool {
        self.external_app_availability.is_available(app)
    }

    pub(crate) fn is_loading_external_app_availability(&self) -> bool {
        self.external_app_availability.is_loading()
    }

    pub(crate) fn refresh_external_app_availability(&mut self, cx: &mut Context<Self>) {
        let task = cx.background_spawn(async { ExternalAppAvailability::detect() });

        self.tasks
            .set_external_app_availability_task(cx.spawn(async move |this, cx| {
                let availability = task.await;

                this.update_or_log(
                    cx,
                    "failed to update external app availability",
                    move |view, cx| {
                        view.external_app_availability = availability;
                        view.tasks.clear_external_app_availability_task();
                        cx.notify();
                    },
                );
            }));
    }
}

fn default_app_availability(app: ExternalApp) -> bool {
    cfg!(target_os = "macos") && matches!(app, ExternalApp::Finder | ExternalApp::Terminal)
}
