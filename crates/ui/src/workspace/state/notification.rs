use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

use crate::workspace::notifications::NotificationSink;

// Keys embed versions like head SHAs and timestamps, so old keys stop matching new events
// and only the most recent ones are worth remembering.
const NOTIFICATION_DEDUPE_CAPACITY: usize = 1024;

pub(crate) struct NotificationState {
    pub(crate) notification_sink: Arc<dyn NotificationSink>,
    pub(crate) notification_dedupe: NotificationDedupe,
}

#[derive(Default)]
pub(crate) struct NotificationDedupe {
    keys: HashSet<String>,
    insertion_order: VecDeque<String>,
}

impl NotificationDedupe {
    /// Returns `false` when the key was already seen.
    pub(crate) fn insert(&mut self, key: String) -> bool {
        if self.keys.contains(&key) {
            return false;
        }
        if self.insertion_order.len() == NOTIFICATION_DEDUPE_CAPACITY
            && let Some(oldest) = self.insertion_order.pop_front()
        {
            self.keys.remove(&oldest);
        }
        self.keys.insert(key.clone());
        self.insertion_order.push_back(key);
        true
    }

    pub(crate) fn clear(&mut self) {
        self.keys.clear();
        self.insertion_order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupe_rejects_repeated_keys_and_evicts_oldest_at_capacity() {
        let mut dedupe = NotificationDedupe::default();
        assert!(dedupe.insert("first".to_string()));
        assert!(!dedupe.insert("first".to_string()));

        for index in 1..NOTIFICATION_DEDUPE_CAPACITY {
            assert!(dedupe.insert(index.to_string()));
        }
        assert!(!dedupe.insert("first".to_string()));

        assert!(dedupe.insert("overflow".to_string()));
        assert_eq!(dedupe.keys.len(), NOTIFICATION_DEDUPE_CAPACITY);
        assert!(dedupe.insert("first".to_string()));
    }
}
