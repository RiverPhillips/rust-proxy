use std::io;

use futures_lite::{
    io::{copy, split},
    StreamExt,
};
use glommio::{
    net::{TcpListener, TcpStream},
    GlommioError, LocalExecutorPoolBuilder,
};
use tracing::{debug, error, info};

pub fn run(concurrency: usize) -> Result<(), GlommioError<()>> {
    let handles =
        LocalExecutorPoolBuilder::new(glommio::PoolPlacement::MaxSpread(concurrency, None))
            .on_all_shards(|| async move {
                let id = glommio::executor().id();
                debug!("Starting on executor {}", id);

                let listener = TcpListener::bind("127.0.0.1:2000").unwrap();
                info!("Listening on {}", listener.local_addr().unwrap());
                let mut incoming = listener.incoming();
                while let Some(conn) = incoming.next().await {
                    match conn {
                        Ok(downstream) => {
                            glommio::spawn_local(async move {
                                if let Err(e) = handle_echo_connection(downstream).await {
                                    error!("Connection error: {}", e);
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

async fn handle_echo_connection(downstream: TcpStream) -> Result<(), io::Error> {
    let (downstream_rx, downstream_tx) = split(downstream);

    glommio::spawn_local(copy(downstream_rx, downstream_tx)).await?;

    Ok(())
}
