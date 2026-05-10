use harbor_sync::PullRequestChangeEvent;

use gpui::{AppContext, Context};

use crate::workspace::{AppView, PullRequestInboxMode, async_updates::AppViewAsyncUpdateExt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HarborNotification {
    pub(crate) summary: String,
    pub(crate) body: String,
}

impl HarborNotification {
    pub(crate) fn from_pull_request_change(event: &PullRequestChangeEvent) -> Self {
        Self {
            summary: event.summary(),
            body: event.body(),
        }
    }
}

pub(crate) trait NotificationSink: Send + Sync {
    fn notify(&self, notification: HarborNotification) -> std::result::Result<(), String>;
}

#[derive(Clone, Debug, Default)]
pub(crate) struct NativeNotificationSink;

impl NotificationSink for NativeNotificationSink {
    fn notify(&self, notification: HarborNotification) -> std::result::Result<(), String> {
        notify_rust::Notification::new()
            .appname("Harbor")
            .summary(&notification.summary)
            .body(&notification.body)
            .show()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

impl AppView {
    pub(crate) fn handle_pull_request_change_events(
        &mut self,
        events: Vec<PullRequestChangeEvent>,
        cx: &mut Context<Self>,
    ) {
        if self.sync_runtime.activity_state != harbor_sync::ActivityState::Background
            || !self.notification_state.notifications_enabled
        {
            return;
        }

        for event in events {
            let dedupe_key = event.dedupe_key();
            if !self
                .notification_state
                .notification_dedupe
                .insert(dedupe_key)
            {
                continue;
            }

            let notification = HarborNotification::from_pull_request_change(&event);
            let sink = self.notification_state.notification_sink.clone();
            let task = cx.background_spawn(async move { sink.notify(notification) });

            cx.spawn(async move |this, cx| {
                let result = task.await;
                this.update_or_log(
                    cx,
                    "failed to update notification state",
                    move |view, cx| {
                        if let Err(error) = result {
                            let message = format!("Failed to send notification: {error}");
                            view.repository_state.repository_error = Some(message.clone());
                            view.status = message;
                            cx.notify();
                        }
                    },
                );
            })
            .detach();
        }
    }

    pub(crate) fn catch_up_active_inbox_after_focus(&mut self, cx: &mut Context<Self>) {
        if self.is_loading_prs {
            return;
        }

        if self.active_inbox_focus_catch_up_due()
            && let Some(repository) = self.repository_state.configured_repo.clone()
        {
            if self.pull_request_inbox.mode == PullRequestInboxMode::NeedsReview {
                tracing::info!(
                    repository = %repository.full_name(),
                    mode = self.pull_request_inbox.mode.key(),
                    "github graphql source: focus catch-up inbox refresh"
                );
            }
            self.refresh_pull_requests_light(repository, cx);
        }
    }
}
