mod actions;
mod diff;
mod diff_reviews;
mod fake_data;
mod panels;
mod workspace;

pub use actions::bind_keys;
pub use workspace::AppView;

#[cfg(test)]
mod tests;
