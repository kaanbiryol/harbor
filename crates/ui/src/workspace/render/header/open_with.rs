use gpui::{Anchor, IntoElement, px};
use gpui_component::{
    Disableable, IconName, Sizable,
    button::{Button, DropdownButton},
};
use harbor_git::ExternalApp;

use crate::actions::*;
use crate::workspace::AppView;

pub(super) fn open_with_action(app: ExternalApp) -> Box<dyn gpui::Action> {
    match app {
        ExternalApp::VsCode => Box::new(OpenWithVsCode),
        ExternalApp::Cursor => Box::new(OpenWithCursor),
        ExternalApp::Zed => Box::new(OpenWithZed),
        ExternalApp::Finder => Box::new(OpenWithFinder),
        ExternalApp::Terminal => Box::new(OpenWithTerminal),
        ExternalApp::Ghostty => Box::new(OpenWithGhostty),
        ExternalApp::Warp => Box::new(OpenWithWarp),
        ExternalApp::Xcode => Box::new(OpenWithXcode),
    }
}

pub(super) fn open_with_icon(app: ExternalApp) -> IconName {
    match app {
        ExternalApp::Finder => IconName::FolderOpen,
        ExternalApp::Terminal | ExternalApp::Ghostty | ExternalApp::Warp => {
            IconName::SquareTerminal
        }
        ExternalApp::VsCode | ExternalApp::Cursor | ExternalApp::Zed | ExternalApp::Xcode => {
            IconName::Frame
        }
    }
}

pub(crate) fn open_with_app_disabled(
    has_local_path: bool,
    local_action_running: bool,
    app_available: bool,
) -> bool {
    !has_local_path || local_action_running || !app_available
}

impl AppView {
    pub(super) fn render_open_with_dropdown(&self) -> impl IntoElement {
        let has_repository = self.current_repository().is_some();
        let local_path = self.current_repository_local_path().cloned();
        let has_local_path = local_path.is_some();
        let local_action_running = self.tasks.local_task.is_some();
        let app_availability = ExternalApp::ALL.map(|app| self.external_app_is_available(app));

        DropdownButton::new("open-with")
            .button(
                Button::new("open-with-primary")
                    .icon(IconName::ExternalLink)
                    .label("Open With")
                    .small()
                    .compact(),
            )
            .small()
            .compact()
            .outline()
            .disabled(!has_repository || local_action_running)
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                let mut menu = menu.max_w(px(320.)).menu_with_disabled(
                    "Choose Local Checkout...",
                    Box::new(ChooseLocalCheckout),
                    !has_repository || local_action_running,
                );

                if let Some(local_path) = local_path.clone() {
                    menu = menu.label(format!("Local: {}", local_path.display()));
                } else {
                    menu = menu.label("No local checkout selected");
                }

                menu = menu.separator();

                for (app_index, app) in ExternalApp::ALL.into_iter().enumerate() {
                    let disabled = open_with_app_disabled(
                        has_local_path,
                        local_action_running,
                        app_availability[app_index],
                    );
                    menu = menu.menu_with_icon_and_disabled(
                        app.label(),
                        open_with_icon(app),
                        open_with_action(app),
                        disabled,
                    );
                }

                menu
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disables_open_with_apps_without_local_path() {
        assert!(open_with_app_disabled(false, false, true));
        assert!(open_with_app_disabled(true, true, true));
        assert!(open_with_app_disabled(true, false, false));
        assert!(!open_with_app_disabled(true, false, true));
    }
}
