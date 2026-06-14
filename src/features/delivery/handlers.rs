use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tracing::{info, warn};

use crate::{
    error::AppError,
    features::delivery::{
        models::DeliveryJobResponse,
        repository::DeliveryRepository,
    },
    queue,
    state::AppState,
};

pub async fn retry_delivery_job(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<DeliveryJobResponse>), AppError> {
    let repo = DeliveryRepository::new(state.db);

    let job = match repo.reset_for_retry(&public_id).await? {
        Some(j) => j,
        None => {
            let exists = repo.exists(&public_id).await?;
            if exists {
                return Err(AppError::Conflict(
                    "delivery job is not in a retryable state".into(),
                    vec![],
                ));
            } else {
                return Err(AppError::NotFound(format!(
                    "Delivery job not found: {public_id}"
                )));
            }
        }
    };

    // Re-enqueue; outbox poller covers any XADD failure.
    if let Err(e) = queue::enqueue(&state.redis, &job.stream_name, &job.public_id).await {
        warn!(public_id = %job.public_id, "retry XADD failed, outbox poller will pick up: {e}");
    } else {
        if let Err(e) = queue::register_stream(&state.redis, &job.stream_name).await {
            warn!(stream = %job.stream_name, "register_stream failed on retry: {e}");
        }
        if let Err(e) = repo.mark_enqueued(job.id).await {
            warn!(public_id = %job.public_id, "mark_enqueued after retry failed: {e:?}");
        }
    }

    info!(public_id = %job.public_id, "delivery job queued for retry");
    Ok((StatusCode::OK, Json(DeliveryJobResponse::from(job))))
}
