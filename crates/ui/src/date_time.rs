use chrono::{DateTime, Datelike, Duration, Utc};

pub(crate) fn natural_time_label(time: DateTime<Utc>) -> String {
    natural_time_label_at(time, Utc::now())
}

pub(crate) fn natural_time_label_at(time: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let duration = now.signed_duration_since(time);
    if duration.num_seconds().abs() < 60 {
        return "just now".to_string();
    }

    let future = duration < Duration::zero();
    let distance = if future { -duration } else { duration };
    let distance = natural_time_distance(distance);

    if future {
        format!("in {distance}")
    } else {
        format!("{distance} ago")
    }
}

pub(crate) fn natural_time_label_with_edit(
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
) -> String {
    natural_time_label_with_edit_at(created_at, updated_at, Utc::now())
}

pub(crate) fn natural_time_label_with_edit_at(
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> String {
    let mut label = natural_time_label_at(created_at, now);

    if edited_time(created_at, updated_at).is_some() {
        label.push_str(" (edited)");
    }

    label
}

pub(crate) fn full_time_label(time: DateTime<Utc>) -> String {
    time.format("%Y-%m-%d %H:%M UTC").to_string()
}

pub(crate) fn month_day_label(time: DateTime<Utc>) -> String {
    format!("{} {}", time.format("%b"), time.day())
}

pub(crate) fn full_time_label_with_edit(
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
) -> String {
    let mut label = full_time_label(created_at);

    if let Some(updated_at) = edited_time(created_at, updated_at) {
        label.push_str(" edited ");
        label.push_str(&full_time_label(updated_at));
    }

    label
}

fn natural_time_distance(duration: Duration) -> String {
    let (value, unit) = if duration.num_days() >= 365 {
        (duration.num_days() / 365, "year")
    } else if duration.num_days() >= 30 {
        (duration.num_days() / 30, "month")
    } else if duration.num_days() >= 7 {
        (duration.num_days() / 7, "week")
    } else if duration.num_days() >= 1 {
        (duration.num_days(), "day")
    } else if duration.num_hours() >= 1 {
        (duration.num_hours(), "hour")
    } else {
        (duration.num_minutes(), "minute")
    };

    if value == 1 {
        format!("1 {unit}")
    } else {
        format!("{value} {unit}s")
    }
}

fn edited_time(
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
) -> Option<DateTime<Utc>> {
    updated_at.filter(|updated_at| *updated_at != created_at)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::*;

    #[test]
    fn formats_past_time_as_natural_label() {
        let time = Utc
            .with_ymd_and_hms(2026, 6, 14, 13, 42, 0)
            .single()
            .expect("valid timestamp");
        let now = Utc
            .with_ymd_and_hms(2026, 7, 5, 13, 42, 0)
            .single()
            .expect("valid timestamp");

        assert_eq!(natural_time_label_at(time, now), "3 weeks ago");
    }

    #[test]
    fn formats_future_time_as_natural_label() {
        let now = Utc
            .with_ymd_and_hms(2026, 7, 5, 13, 42, 0)
            .single()
            .expect("valid timestamp");

        assert_eq!(
            natural_time_label_at(now + Duration::hours(2), now),
            "in 2 hours"
        );
    }

    #[test]
    fn marks_edited_time_label() {
        let created_at = Utc
            .with_ymd_and_hms(2026, 7, 5, 13, 42, 0)
            .single()
            .expect("valid timestamp");
        let now = created_at + Duration::minutes(30);

        assert_eq!(
            natural_time_label_with_edit_at(
                created_at,
                Some(created_at + Duration::minutes(5)),
                now
            ),
            "30 minutes ago (edited)"
        );
    }

    #[test]
    fn formats_full_time_label_with_edit() {
        let created_at = Utc
            .with_ymd_and_hms(2026, 6, 14, 13, 42, 0)
            .single()
            .expect("valid timestamp");

        assert_eq!(
            full_time_label_with_edit(created_at, Some(created_at + Duration::minutes(5))),
            "2026-06-14 13:42 UTC edited 2026-06-14 13:47 UTC"
        );
    }

    #[test]
    fn formats_month_day_label() {
        let time = Utc
            .with_ymd_and_hms(2026, 5, 10, 13, 42, 0)
            .single()
            .expect("valid timestamp");

        assert_eq!(month_day_label(time), "May 10");
    }
}
