use std::time::Duration;

use futures::future::join_all;
use futures_lite::{
    io::{self, copy, split},
    stream::StreamExt,
    FutureExt,
};
use glommio::{
    net::{TcpListener, TcpStream},
    timer::Timer,
    DefaultStallDetectionHandler, GlommioError, LocalExecutorPoolBuilder,
};
use tracing::{debug, error, info};

pub fn run(concurrency: usize) -> Result<(), GlommioError<()>> {
    let handles =
        LocalExecutorPoolBuilder::new(glommio::PoolPlacement::MaxSpread(concurrency, None))
            .detect_stalls(Some(Box::new(|| Box::new(DefaultStallDetectionHandler {}))))
            .on_all_shards(|| async move {
                let id = glommio::executor().id();
                debug!("Starting on executor {}", id);

                let listener = TcpListener::bind("127.0.0.1:8000").unwrap();
                info!("Listening on {}", listener.local_addr().unwrap());
                let mut incoming = listener.incoming();
                while let Some(conn) = incoming.next().await {
                    match conn {
                        Ok(downstream) => {
                            glommio::spawn_local(async move {
                                if let Err(e) = handle_proxy_connection(downstream).await {
                                    error!("Error handling connection: {}", e);
                                }
                            })
                            .detach();
                        }
                        Err(e) => {
                            error!("Accept error: {}", e);
                            break;
                        }
                    }
                }
            })?;

    handles.join_all();
    Ok(())
}

async fn handle_proxy_connection(downstream: TcpStream) -> Result<(), io::Error> {
    let timeout = async {
        Timer::new(Duration::from_millis(250)).await;
        Err(GlommioError::IoError(io::Error::new(
            io::ErrorKind::TimedOut,
            "Timed out connecting to upstream",
        )))
    };
    // Establish upstream connection
    let upstream = TcpStream::connect("127.0.0.1:2000").or(timeout).await?;

    let (upstream_rx, upstream_tx) = split(upstream);
    let (downstream_rx, downstream_tx) = split(downstream);

    join_all(vec![
        glommio::spawn_local(copy(upstream_rx, downstream_tx)),
        glommio::spawn_local(copy(downstream_rx, upstream_tx)),
    ])
    .await;

    Ok(())
}
