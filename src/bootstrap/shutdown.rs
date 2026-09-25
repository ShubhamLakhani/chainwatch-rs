//! Graceful shutdown on Ctrl+C (SIGINT) and SIGTERM.

use tokio_util::sync::CancellationToken;
use tracing::info;

pub fn spawn_signal_handler(token: CancellationToken) {
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        info!("shutdown signal received, stopping indexer and HTTP server");
        token.cancel();
    });
}

pub async fn wait_for_shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm = match tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate(),
        ) {
            Ok(handler) => handler,
            Err(error) => {
                tracing::warn!(%error, "could not register SIGTERM handler; waiting for Ctrl+C only");
                let _ = ctrl_c.await;
                return;
            }
        };
        tokio::select! {
            result = ctrl_c => {
                if let Err(error) = result {
                    tracing::warn!(%error, "Ctrl+C handler failed");
                } else {
                    info!("received SIGINT");
                }
            }
            _ = sigterm.recv() => {
                info!("received SIGTERM");
            }
        }
    }

    #[cfg(not(unix))]
    {
        if let Err(error) = ctrl_c.await {
            tracing::warn!(%error, "Ctrl+C handler failed");
        } else {
            info!("received Ctrl+C");
        }
    }
}
