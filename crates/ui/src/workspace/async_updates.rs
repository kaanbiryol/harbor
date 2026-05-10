use gpui::{AppContext, Context, WeakEntity, Window};

use crate::workspace::{AppView, log_entity_update_error};

pub(crate) trait AppViewAsyncUpdateExt {
    fn update_or_log<C, R>(
        &self,
        cx: &mut C,
        context: &'static str,
        update: impl FnOnce(&mut AppView, &mut Context<AppView>) -> R,
    ) -> Option<R>
    where
        C: AppContext;

    fn update_in_or_log<C, R>(
        &self,
        cx: &mut C,
        context: &'static str,
        update: impl FnOnce(&mut AppView, &mut Window, &mut Context<AppView>) -> R,
    ) -> Option<R>
    where
        C: AppContext;
}

impl AppViewAsyncUpdateExt for WeakEntity<AppView> {
    fn update_or_log<C, R>(
        &self,
        cx: &mut C,
        context: &'static str,
        update: impl FnOnce(&mut AppView, &mut Context<AppView>) -> R,
    ) -> Option<R>
    where
        C: AppContext,
    {
        match self.update(cx, update) {
            Ok(result) => Some(result),
            Err(error) => {
                log_entity_update_error(context, error);
                None
            }
        }
    }

    fn update_in_or_log<C, R>(
        &self,
        cx: &mut C,
        context: &'static str,
        update: impl FnOnce(&mut AppView, &mut Window, &mut Context<AppView>) -> R,
    ) -> Option<R>
    where
        C: AppContext,
    {
        match self.update_in(cx, update) {
            Ok(result) => Some(result),
            Err(error) => {
                log_entity_update_error(context, error);
                None
            }
        }
    }
}
