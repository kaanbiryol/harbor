use harbor_domain::RepoId;
use harbor_sync::PullRequestChangeEvent;

use futures::{FutureExt, channel::oneshot, future::BoxFuture};
use gpui::Context;
use std::sync::OnceLock;

use crate::workspace::{AppView, PullRequestInboxMode, async_updates::AppViewAsyncUpdateExt};

// Must match CFBundleIdentifier in script/package-macos.
#[cfg(target_os = "macos")]
const MACOS_HARBOR_BUNDLE_IDENTIFIER: &str = "com.kaanbiryol.harbor";
// Unbundled builds have no identity of their own, and macOS only delivers notifications
// on behalf of an installed application.
#[cfg(target_os = "macos")]
const MACOS_UNBUNDLED_NOTIFICATION_BUNDLE_IDENTIFIER: &str = "com.apple.finder";
const GROUPED_NOTIFICATION_PULL_REQUEST_LIMIT: usize = 3;
const NOTIFICATION_DEFAULT_ACTION: &str = "default";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HarborNotification {
    pub(crate) summary: String,
    pub(crate) body: String,
    /// Set when every grouped change belongs to one pull request, so a click can open it.
    pub(crate) pull_request: Option<NotificationPullRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NotificationPullRequest {
    pub(crate) repo: RepoId,
    pub(crate) number: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum NotificationResponse {
    Clicked,
    #[default]
    Closed,
}

impl HarborNotification {
    pub(crate) fn from_pull_request_changes(events: &[PullRequestChangeEvent]) -> Option<Self> {
        let mut groups: Vec<(&PullRequestChangeEvent, Vec<&'static str>)> = Vec::new();
        for event in events {
            match groups
                .iter_mut()
                .find(|(first, _)| first.repo == event.repo && first.number == event.number)
            {
                Some((_, labels)) => labels.push(event.kind.label()),
                None => groups.push((event, vec![event.kind.label()])),
            }
        }

        match groups.as_slice() {
            [] => None,
            [(event, labels)] => {
                let (summary, body) = match labels.as_slice() {
                    [_] => (event.summary(), event.body()),
                    labels => (
                        format!("{} updates on #{}", labels.len(), event.number),
                        format!("{}\n{}", event.body(), labels.join(", ")),
                    ),
                };
                Some(Self {
                    summary,
                    body,
                    pull_request: Some(NotificationPullRequest {
                        repo: event.repo.clone(),
                        number: event.number,
                    }),
                })
            }
            groups => {
                let mut lines = groups
                    .iter()
                    .take(GROUPED_NOTIFICATION_PULL_REQUEST_LIMIT)
                    .map(|(event, labels)| {
                        format!("#{} {}: {}", event.number, labels.join(", "), event.title)
                    })
                    .collect::<Vec<_>>();
                let remaining = groups
                    .len()
                    .saturating_sub(GROUPED_NOTIFICATION_PULL_REQUEST_LIMIT);
                if remaining > 0 {
                    lines.push(format!("and {remaining} more"));
                }
                Some(Self {
                    summary: format!("{} pull requests updated", groups.len()),
                    body: lines.join("\n"),
                    pull_request: None,
                })
            }
        }
    }
}

pub(crate) trait NotificationSink: Send + Sync {
    /// Shows the notification and resolves once the user clicks or closes it.
    fn notify(
        &self,
        notification: HarborNotification,
    ) -> BoxFuture<'static, std::result::Result<NotificationResponse, String>>;
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct RecordingNotificationSink {
    pub(crate) notifications: std::sync::Mutex<Vec<HarborNotification>>,
    pub(crate) response: NotificationResponse,
}

#[cfg(test)]
impl NotificationSink for RecordingNotificationSink {
    fn notify(
        &self,
        notification: HarborNotification,
    ) -> BoxFuture<'static, std::result::Result<NotificationResponse, String>> {
        let result = self
            .notifications
            .lock()
            .map(|mut notifications| {
                notifications.push(notification);
                self.response
            })
            .map_err(|error| error.to_string());
        futures::future::ready(result).boxed()
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct NativeNotificationSink;

impl NativeNotificationSink {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl NotificationSink for NativeNotificationSink {
    fn notify(
        &self,
        notification: HarborNotification,
    ) -> BoxFuture<'static, std::result::Result<NotificationResponse, String>> {
        let (sender, receiver) = oneshot::channel();
        // Waiting for a response can block until the user clears the notification, so each
        // one gets its own thread instead of occupying a shared executor worker.
        let spawned = std::thread::Builder::new()
            .name("harbor-notification".to_string())
            .spawn(move || {
                if sender
                    .send(show_native_notification(&notification))
                    .is_err()
                {
                    tracing::debug!("notification response arrived after its receiver dropped");
                }
            });

        async move {
            spawned.map_err(|error| format!("Failed to start notification thread: {error}"))?;
            receiver
                .await
                .map_err(|_| "Notification thread stopped without a response".to_string())?
        }
        .boxed()
    }
}

fn show_native_notification(
    notification: &HarborNotification,
) -> std::result::Result<NotificationResponse, String> {
    configure_native_notifications()?;

    let handle = notify_rust::Notification::new()
        .appname("Harbor")
        .summary(&notification.summary)
        .body(&notification.body)
        .show()
        .map_err(|error| error.to_string())?;

    let mut response = NotificationResponse::Closed;
    handle.wait_for_action(|action| {
        if action == NOTIFICATION_DEFAULT_ACTION {
            response = NotificationResponse::Clicked;
        }
    });
    Ok(response)
}

fn configure_native_notifications() -> std::result::Result<(), String> {
    static CONFIGURATION: OnceLock<std::result::Result<(), String>> = OnceLock::new();

    CONFIGURATION
        .get_or_init(configure_native_notifications_once)
        .clone()
}

#[cfg(target_os = "macos")]
fn configure_native_notifications_once() -> std::result::Result<(), String> {
    let bundle_identifier = if is_running_from_app_bundle() {
        MACOS_HARBOR_BUNDLE_IDENTIFIER
    } else {
        MACOS_UNBUNDLED_NOTIFICATION_BUNDLE_IDENTIFIER
    };
    notify_rust::set_application(bundle_identifier)
        .map_err(|error| format!("Failed to configure macOS notification application: {error}"))
}

// Claiming Harbor's identity from an unbundled build would make clicks launch an installed
// Harbor.app instead of returning to this process.
#[cfg(target_os = "macos")]
fn is_running_from_app_bundle() -> bool {
    std::env::current_exe().is_ok_and(|executable| {
        executable
            .parent()
            .filter(|directory| directory.ends_with("Contents/MacOS"))
            .and_then(|directory| directory.parent()?.parent())
            .is_some_and(|bundle| {
                bundle
                    .extension()
                    .is_some_and(|extension| extension == "app")
            })
    })
}

#[cfg(not(target_os = "macos"))]
fn configure_native_notifications_once() -> std::result::Result<(), String> {
    Ok(())
}

impl AppView {
    pub(crate) fn handle_pull_request_change_events(
        &mut self,
        events: Vec<PullRequestChangeEvent>,
        cx: &mut Context<Self>,
    ) {
        let auth_generation = self.auth_generation;
        let preferences = &self.preferences.value.notifications;
        if !self.sync_runtime.is_background() || !preferences.enabled {
            return;
        }

        let events = events
            .into_iter()
            .filter(|event| {
                event.kind.enabled_by(preferences)
                    && self
                        .notification_state
                        .notification_dedupe
                        .insert(event.dedupe_key())
            })
            .collect::<Vec<_>>();
        let Some(notification) = HarborNotification::from_pull_request_changes(&events) else {
            return;
        };

        let pull_request = notification.pull_request.clone();
        let response = self
            .notification_state
            .notification_sink
            .notify(notification);
        cx.spawn(async move |this, cx| {
            let response = response.await;
            this.update_in_session_or_log(
                cx,
                auth_generation,
                "failed to update notification state",
                move |view, cx| match response {
                    Ok(NotificationResponse::Clicked) => {
                        view.open_notification_pull_request(pull_request, cx);
                    }
                    Ok(NotificationResponse::Closed) => {}
                    Err(error) => {
                        let message = format!("Failed to send notification: {error}");
                        view.repository_state.set_error(message.clone());
                        view.report_error(message, cx);
                        cx.notify();
                    }
                },
            );
        })
        .detach();
    }

    fn open_notification_pull_request(
        &mut self,
        pull_request: Option<NotificationPullRequest>,
        cx: &mut Context<Self>,
    ) {
        cx.activate(true);
        if let Some(index) = pull_request.and_then(|target| {
            self.pull_requests.iter().position(|pull_request| {
                pull_request.repo == target.repo && pull_request.number == target.number
            })
        }) {
            self.select_pull_request(index, cx);
        }
    }

    pub(crate) fn catch_up_active_inbox_after_focus(&mut self, cx: &mut Context<Self>) {
        if self.pull_request_inbox.is_loading() || self.pull_request_inbox.is_loading_more() {
            return;
        }

        if self.active_inbox_focus_catch_up_due()
            && let Some(repository) = self.repository_state.configured_repo_cloned()
        {
            if self.pull_request_inbox.mode() == PullRequestInboxMode::NeedsReview {
                tracing::info!(
                    repository = %repository.full_name(),
                    mode = self.pull_request_inbox.mode().key(),
                    "github graphql source: focus catch-up inbox refresh"
                );
            }
            self.refresh_pull_requests_light(repository, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use harbor_sync::PullRequestChangeKind;

    use super::*;

    #[test]
    fn groups_changes_on_one_pull_request_into_one_notification() {
        let notification = HarborNotification::from_pull_request_changes(&[
            event(7, PullRequestChangeKind::CheckFailed),
            event(7, PullRequestChangeKind::HeadChanged),
        ]);

        assert_eq!(
            notification,
            Some(HarborNotification {
                summary: "2 updates on #7".to_string(),
                body: "acme/app: Title 7\nChecks failed, Pull request updated".to_string(),
                pull_request: Some(NotificationPullRequest {
                    repo: RepoId::new("acme", "app"),
                    number: 7,
                }),
            })
        );
    }

    #[test]
    fn groups_changes_across_pull_requests_without_click_target() {
        let notification = HarborNotification::from_pull_request_changes(&[
            event(7, PullRequestChangeKind::CheckFailed),
            event(8, PullRequestChangeKind::Approved),
            event(7, PullRequestChangeKind::HeadChanged),
        ]);

        assert_eq!(
            notification,
            Some(HarborNotification {
                summary: "2 pull requests updated".to_string(),
                body: "#7 Checks failed, Pull request updated: Title 7\n#8 Pull request approved: Title 8"
                    .to_string(),
                pull_request: None,
            })
        );
        assert_eq!(HarborNotification::from_pull_request_changes(&[]), None);
    }

    fn event(number: u64, kind: PullRequestChangeKind) -> PullRequestChangeEvent {
        PullRequestChangeEvent {
            repo: RepoId::new("acme", "app"),
            number,
            title: format!("Title {number}"),
            kind,
            version: "v1".to_string(),
        }
    }
}
