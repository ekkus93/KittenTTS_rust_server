use axum::middleware;
use axum::Router;

use crate::app_state::AppState;
use crate::middleware::{auth, request_context};

pub(crate) mod health;
pub(crate) mod tts;
pub(crate) mod voices;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(health::routes())
        .merge(voices::routes())
        .merge(tts::routes())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::authorize,
        ))
        .layer(middleware::from_fn(request_context::track_request))
        .with_state(state)
}
