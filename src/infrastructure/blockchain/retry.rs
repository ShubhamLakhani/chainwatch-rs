use crate::domain::AppError;
use crate::infrastructure::telemetry::metrics::Metrics;
use std::future::Future;
use std::time::Duration;
use tracing::warn;

/// Retry a fallible async operation with exponential backoff.
///
/// The first attempt is immediate. Subsequent attempts wait 100ms, 200ms,
/// 400ms, ... capped at 2 seconds.
pub async fn retry_with_backoff<T, F, Fut>(
    max_retries: u32,
    timeout: Duration,
    metrics: &Metrics,
    mut operation: F,
) -> Result<T, AppError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, AppError>>,
{
    let mut delay = Duration::from_millis(100);
    let mut last_error: Option<AppError> = None;

    for attempt in 0..=max_retries {
        match tokio::time::timeout(timeout, operation()).await {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(error)) => {
                metrics.incr_rpc_failure();
                last_error = Some(error);
            }
            Err(_) => {
                metrics.incr_rpc_failure();
                last_error = Some(AppError::RpcTimeout(timeout));
            }
        }

        if attempt < max_retries {
            metrics.incr_retry();
            let error = last_error
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default();
            warn!(attempt = attempt + 1, %error, delay_ms = delay.as_millis(), "retrying RPC call");
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(Duration::from_secs(2));
        }
    }

    Err(last_error.unwrap_or_else(|| AppError::rpc("retry loop exhausted")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn retries_until_success() {
        let metrics = Metrics::new();
        let attempts = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let attempts_clone = Arc::clone(&attempts);
        let result = retry_with_backoff(3, Duration::from_secs(1), &metrics, || {
            let attempts_clone = Arc::clone(&attempts_clone);
            async move {
                let n = attempts_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if n < 2 {
                    Err(AppError::rpc("temporary"))
                } else {
                    Ok(7u64)
                }
            }
        })
        .await
        .expect("should succeed after retries");
        assert_eq!(result, 7);
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 3);
        let snap = metrics.snapshot();
        assert_eq!(snap.rpc_request_failures, 2);
        assert_eq!(snap.retry_count, 2);
    }

    #[tokio::test]
    async fn retries_exhaust_and_return_last_error() {
        let metrics = Metrics::new();
        let err = retry_with_backoff(1, Duration::from_millis(50), &metrics, || async {
            Err::<(), _>(AppError::rpc("still down"))
        })
        .await
        .expect_err("should fail");
        assert!(err.to_string().contains("still down"));
        assert_eq!(metrics.snapshot().retry_count, 1);
    }
}
