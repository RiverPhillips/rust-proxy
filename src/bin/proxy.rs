use std::{thread::available_parallelism, time::Duration};

use futures::future::join_all;
use futures_lite::{
    io::{self, copy, split},
    stream::StreamExt,
    FutureExt,
};
use glommio::{
    net::{TcpListener, TcpStream},
    timer::Timer,
    GlommioError, LocalExecutorPoolBuilder,
};

fn main() -> Result<(), GlommioError<()>> {
    env_logger::init();

    let default_parrallelism: usize = match available_parallelism() {
        Ok(parallelism) => parallelism.into(),
        Err(e) => {
            log::error!("Failed to get available parallelism: {}. Starting on 1 core", e);
            1
        }
    };

    let handles = LocalExecutorPoolBuilder::new(glommio::PoolPlacement::MaxSpread(
        default_parrallelism,
        None,
    ))
    .on_all_shards(|| async move {
        let id = glommio::executor().id();
        log::debug!("Starting on executor {}", id);

        let listener = TcpListener::bind("127.0.0.1:8000").unwrap();
        log::info!("Listening on {}", listener.local_addr().unwrap());
        let mut incoming = listener.incoming();
        while let Some(conn) = incoming.next().await {
            match conn {
                Ok(downstream) => {
                    glommio::spawn_local(async move {
                        if let Err(e) = handle_connection(downstream).await {
                            log::error!("Connection error: {}", e);
                        }
                    })
                    .detach();
                }
                Err(e) => {
                    log::error!("Accept error: {}", e);
                    break;
                }
            }
        }
    })?;

    handles.join_all();
    Ok(())
}

async fn handle_connection(downstream: TcpStream) -> Result<(), io::Error> {
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
