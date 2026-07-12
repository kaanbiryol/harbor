use gpui::{Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};
use harbor_domain::{Label, PullRequestMetadataOptions, PullRequestPerson};

use crate::{
    actions::{PullRequestMetadataField, PullRequestMetadataRequest},
    workspace::{AppView, async_updates::AppViewAsyncUpdateExt},
};

#[derive(Default)]
pub(crate) struct PullRequestMetadataOptionsState {
    repository: Option<(String, String)>,
    pub(crate) options: PullRequestMetadataOptions,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
}

impl AppView {
    pub(super) fn load_pull_request_metadata_options(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pull_request) = self.selected_pull_request() else {
            return;
        };
        let repository = (
            pull_request.repo.owner.clone(),
            pull_request.repo.name.clone(),
        );
        if (self.pull_request_metadata_options.loading
            && self.pull_request_metadata_options.repository.as_ref() == Some(&repository))
            || (self.pull_request_metadata_options.repository.as_ref() == Some(&repository)
                && self.pull_request_metadata_options.error.is_none())
        {
            return;
        }

        if self.pull_request_metadata_options.repository.as_ref() != Some(&repository) {
            self.pull_request_metadata_options.options = Default::default();
        }
        self.pull_request_metadata_options.loading = true;
        self.pull_request_metadata_options.error = None;
        self.pull_request_metadata_options.repository = Some(repository.clone());
        cx.notify();
        let github_api = self.github_api.clone();

        cx.spawn_in(window, async move |this, cx| {
            let result = github_api
                .list_pull_request_metadata_options(&repository.0, &repository.1)
                .await;
            this.update_in_or_log(
                cx,
                "failed to update pull request metadata options",
                move |view, _, cx| {
                    if view.pull_request_metadata_options.repository.as_ref() != Some(&repository) {
                        return;
                    }
                    view.pull_request_metadata_options.loading = false;
                    match result {
                        Ok(options) => {
                            view.pull_request_metadata_options.options = options;
                            view.pull_request_metadata_options.error = None;
                        }
                        Err(error) => {
                            view.pull_request_metadata_options.error =
                                Some(format!("Failed to load choices: {error}"));
                        }
                    }
                    cx.notify();
                },
            );
        })
        .detach();
    }

    pub(crate) fn on_pull_request_metadata_input_event(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let field = self.pull_request_metadata_field_for_input(input);

        match event {
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } => {
                if let Some(field) = field {
                    self.add_pull_request_metadata(field, window, cx);
                }
            }
            _ => {}
        }
    }

    pub(super) fn add_pull_request_metadata(
        &mut self,
        field: PullRequestMetadataField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.action_runtime.pull_request_metadata_action_running() {
            self.status = "A pull request metadata update is already running".to_string();
            cx.notify();
            return;
        }

        let request = match self.pull_request_metadata_request(field, cx) {
            Ok(request) => request,
            Err(message) => {
                self.status = message;
                cx.notify();
                return;
            }
        };

        self.action_runtime
            .start_pull_request_metadata_action(field);
        self.status = request.start_status();
        cx.notify();
        let github_api = self.github_api.clone();

        cx.spawn_in(window, async move |this, cx| {
            let result = match request.field {
                PullRequestMetadataField::Reviewer => {
                    github_api
                        .request_pull_request_reviewer(
                            &request.owner,
                            &request.repo,
                            request.number,
                            &request.value,
                        )
                        .await
                }
                PullRequestMetadataField::Assignee => {
                    github_api
                        .add_pull_request_assignee(
                            &request.owner,
                            &request.repo,
                            request.number,
                            &request.value,
                        )
                        .await
                }
                PullRequestMetadataField::Label => {
                    github_api
                        .add_pull_request_label(
                            &request.owner,
                            &request.repo,
                            request.number,
                            &request.value,
                        )
                        .await
                }
            };

            this.update_in_or_log(
                cx,
                "failed to update pull request metadata action state",
                move |view, window, cx| {
                    match result {
                        Ok(()) => {
                            view.action_runtime.finish_pull_request_metadata_action();
                            view.apply_pull_request_metadata_addition(&request);
                            view.pull_request_metadata_input(request.field)
                                .update(cx, |input, cx| input.set_value("", window, cx));
                            view.status = request.success_status();
                        }
                        Err(error) => {
                            let message = format!("Failed to {}: {error}", request.failure_label());
                            view.action_runtime
                                .finish_pull_request_metadata_action_failure(message.clone());
                            view.status = message;
                        }
                    }

                    cx.notify();
                },
            );
        })
        .detach();
    }

    fn pull_request_metadata_request(
        &self,
        field: PullRequestMetadataField,
        cx: &Context<Self>,
    ) -> Result<PullRequestMetadataRequest, String> {
        let Some(pull_request) = self.selected_pull_request() else {
            return Err("Select a pull request before editing metadata".to_string());
        };
        let input = self.pull_request_metadata_input(field);
        let value = normalize_metadata_value(field, input.read(cx).value().as_ref());
        if value.is_empty() {
            return Err(format!("Enter a {} to add", field.name()));
        }
        if pull_request_has_metadata_value(pull_request, field, &value) {
            return Err(format!("{} is already a {}", value, field.name()));
        }

        Ok(PullRequestMetadataRequest {
            field,
            owner: pull_request.repo.owner.clone(),
            repo: pull_request.repo.name.clone(),
            number: pull_request.number,
            value,
        })
    }

    fn apply_pull_request_metadata_addition(&mut self, request: &PullRequestMetadataRequest) {
        let Some(pull_request) = self.pull_requests.iter_mut().find(|pull_request| {
            pull_request.repo.owner == request.owner
                && pull_request.repo.name == request.repo
                && pull_request.number == request.number
        }) else {
            return;
        };

        match request.field {
            PullRequestMetadataField::Reviewer => {
                pull_request.requested_reviewers.push(PullRequestPerson {
                    login: request.value.clone(),
                    avatar_url: None,
                });
            }
            PullRequestMetadataField::Assignee => {
                pull_request.assignees.push(PullRequestPerson {
                    login: request.value.clone(),
                    avatar_url: None,
                });
            }
            PullRequestMetadataField::Label => {
                pull_request.labels.push(Label {
                    name: request.value.clone(),
                    color: None,
                });
            }
        }
    }

    pub(super) fn pull_request_metadata_input(
        &self,
        field: PullRequestMetadataField,
    ) -> Entity<InputState> {
        match field {
            PullRequestMetadataField::Reviewer => self.pull_request_reviewer_input.clone(),
            PullRequestMetadataField::Assignee => self.pull_request_assignee_input.clone(),
            PullRequestMetadataField::Label => self.pull_request_label_input.clone(),
        }
    }

    fn pull_request_metadata_field_for_input(
        &self,
        input: &Entity<InputState>,
    ) -> Option<PullRequestMetadataField> {
        [
            PullRequestMetadataField::Reviewer,
            PullRequestMetadataField::Assignee,
            PullRequestMetadataField::Label,
        ]
        .into_iter()
        .find(|field| self.pull_request_metadata_input(*field).entity_id() == input.entity_id())
    }
}

fn normalize_metadata_value(field: PullRequestMetadataField, value: &str) -> String {
    let value = value.trim();
    match field {
        PullRequestMetadataField::Reviewer | PullRequestMetadataField::Assignee => {
            value.trim_start_matches('@').trim().to_string()
        }
        PullRequestMetadataField::Label => value.to_string(),
    }
}

fn pull_request_has_metadata_value(
    pull_request: &harbor_domain::PullRequest,
    field: PullRequestMetadataField,
    value: &str,
) -> bool {
    match field {
        PullRequestMetadataField::Reviewer => pull_request
            .requested_reviewers
            .iter()
            .any(|reviewer| reviewer.login.eq_ignore_ascii_case(value)),
        PullRequestMetadataField::Assignee => pull_request
            .assignees
            .iter()
            .any(|assignee| assignee.login.eq_ignore_ascii_case(value)),
        PullRequestMetadataField::Label => pull_request
            .labels
            .iter()
            .any(|label| label.name.eq_ignore_ascii_case(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_metadata_value;
    use crate::actions::PullRequestMetadataField;

    #[test]
    fn normalizes_metadata_input_values() {
        assert_eq!(
            normalize_metadata_value(PullRequestMetadataField::Reviewer, "  @octocat "),
            "octocat"
        );
        assert_eq!(
            normalize_metadata_value(PullRequestMetadataField::Label, " needs review "),
            "needs review"
        );
    }
}
