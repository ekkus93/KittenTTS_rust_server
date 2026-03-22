use axum::Router;

use crate::app_state::AppState;

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
}
