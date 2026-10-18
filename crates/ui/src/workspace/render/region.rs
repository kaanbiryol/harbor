use gpui::{
    AnyElement, Context, Entity, IntoElement, Render, Subscription, WeakEntity, Window, div,
    prelude::*,
};

use crate::workspace::{AppView, log_entity_update_error};

type RenderRegion = fn(&mut AppView, &mut Context<AppView>) -> AnyElement;

/// A cached slice of the workspace rendered as its own view. Scrolling a list notifies
/// the view that renders it, so keeping each scrollable area in its own view means a
/// scroll frame only rebuilds that area instead of the whole workspace.
pub(crate) struct WorkspaceRegion {
    workspace: WeakEntity<AppView>,
    render: RenderRegion,
    _subscription: Subscription,
}

impl WorkspaceRegion {
    fn new(workspace: &Entity<AppView>, render: RenderRegion, cx: &mut Context<Self>) -> Self {
        Self {
            workspace: workspace.downgrade(),
            render,
            _subscription: cx.observe(workspace, |_, _, cx| cx.notify()),
        }
    }
}

impl Render for WorkspaceRegion {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.workspace.update(cx, self.render) {
            Ok(content) => Some(content),
            Err(error) => {
                log_entity_update_error("failed to render workspace region", error);
                None
            }
        };

        div().size_full().flex().flex_col().children(content)
    }
}

pub(crate) struct WorkspaceRegions {
    pub(super) inbox: Entity<WorkspaceRegion>,
    pub(super) changed_files: Entity<WorkspaceRegion>,
    pub(super) panel: Entity<WorkspaceRegion>,
}

impl WorkspaceRegions {
    pub(super) fn new(cx: &mut Context<AppView>) -> Self {
        let workspace = cx.entity();
        Self {
            inbox: cx.new(|cx| {
                WorkspaceRegion::new(
                    &workspace,
                    |view, cx| view.render_inbox(cx).into_any_element(),
                    cx,
                )
            }),
            changed_files: cx.new(|cx| {
                WorkspaceRegion::new(
                    &workspace,
                    |view, cx| view.render_changed_files_sidebar(cx).into_any_element(),
                    cx,
                )
            }),
            panel: cx.new(|cx| {
                WorkspaceRegion::new(
                    &workspace,
                    |view, cx| {
                        let pull_request = view.selected_pull_request().cloned();
                        view.render_panel(pull_request.as_ref(), cx)
                            .into_any_element()
                    },
                    cx,
                )
            }),
        }
    }
}
