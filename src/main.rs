use std::time::Duration;

use futures::future::join_all;
use futures_lite::{stream::StreamExt, io::{split, copy, self}, FutureExt};
use glommio::{net::{TcpListener, TcpStream}, LocalExecutor, timer::Timer};

fn main() {
    let ex = LocalExecutor::default();
    ex.run(async move {
        let listener = TcpListener::bind("127.0.0.1:8000").unwrap();
        println!("Listening on {}", listener.local_addr().unwrap());
        let mut incoming = listener.incoming();
        while let Some(conn) = incoming.next().await {
            match conn {
                Ok(downstream) => {
                    glommio::spawn_local(async move {
                        handle_connection(downstream).await;
                    }).detach();
                }
                Err(e) => {
                    println!("Accept error: {}", e);
                    break;
                }
            }
        }
    })
}


async fn handle_connection(downstream: TcpStream) {                    
    let timeout = async {
        Timer::new(Duration::from_millis(250)).await;
        Err(io::Error::new(io::ErrorKind::TimedOut, "Timed out").into())
    };
    // Establish upstream connection
    let upstream = TcpStream::connect("127.0.0.1:8080").or(timeout).await.unwrap();

    let (upstream_rx, upstream_tx) = split(upstream);
    let (downstream_rx, downstream_tx) = split(downstream);

    let tasks = vec![
        glommio::spawn_local(async move {
            match copy(upstream_rx, downstream_tx).await {
                Ok(_) => {},
                Err(e) => println!("upstream error: {}", e),
            };    
        }),
        glommio::spawn_local(async move {
            match copy(downstream_rx, upstream_tx).await {
                Ok(_) => {},
                Err(e) => println!("downstream error: {}", e),
            }
        }),
    ];      

    join_all(tasks).await;
}