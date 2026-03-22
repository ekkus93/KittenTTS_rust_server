use axum::Router;

use crate::app_state::AppState;

pub(crate) mod health;
pub(crate) mod tts;
pub(crate) mod voices;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(health::routes())
        .merge(voices::routes())
        .merge(tts::routes())
        .with_state(state)
}
