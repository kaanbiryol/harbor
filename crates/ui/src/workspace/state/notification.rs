use std::{collections::HashSet, sync::Arc};

use crate::workspace::notifications::NotificationSink;

pub(crate) struct NotificationState {
    pub(crate) notification_sink: Arc<dyn NotificationSink>,
    pub(crate) notification_dedupe: HashSet<String>,
    pub(crate) notifications_enabled: bool,
}
