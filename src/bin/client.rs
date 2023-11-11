
use futures_channel::mpsc::unbounded;
use futures_util::{future, pin_mut, SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;


#[tokio::main]
async fn main() {
    let connection_url = url::Url::parse("ws://127.0.0.1:8080/").expect("Invalid URL");
    let (stdin_tx, stdin_rx) = unbounded();

    tokio::spawn(
        async move {
            let mut stdin = tokio::io::stdin();
            loop {
                let mut buffer = vec![0; 1024];
                let n = match stdin.read(&mut buffer).await {
                    Err(_) | Ok(0) => break,
                    Ok(n) => n,
                };
                buffer.truncate(n);
                stdin_tx.unbounded_send(Message::binary(buffer)).unwrap()
            }
        }
    );

    let (ws_stream, _) =  connect_async(connection_url).await.expect("Uable to connect to the url");
    println!("WebSocket handshake has been successfully completed");

    let (write, read) = ws_stream.split();
    let stdin_to_ws = stdin_rx.map(Ok).forward(write);
    let ws_to_stdout = {
        read.for_each(|message| async {
            let data = message.unwrap().into_data();
            tokio::io::stdout().write_all(&data).await.unwrap()
        })
    };

    pin_mut!(stdin_to_ws, ws_to_stdout);
    future::select(stdin_to_ws, ws_to_stdout).await;
}