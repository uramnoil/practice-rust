use std::{net::{SocketAddr}, sync::Arc, borrow::Borrow};
use futures::StreamExt;
use tokio::{io::{AsyncReadExt}, net::{TcpListener, TcpStream}, sync::{broadcast::{self, Sender}, Mutex}};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    let (message_tx, mut message_rx) = broadcast::channel::<String>(10);
    let (close_tx, mut close_rx) = broadcast::channel::<SocketAddr>(10);
    let mut clients: Vec<Arc<Mutex<TcpStream>>> = vec![];

    // 接続待機
    loop {
        tokio::select! {
            // 接続要求時
            result = listener.accept() => {
                match result {
                    Ok((stream, addr)) => {
                        let shared_stream = Arc::new(Mutex::new(stream));
                        clients.push(shared_stream.clone());
                        let message_tx_clone = message_tx.clone();
                        let close_tx_clone = close_tx.clone();
                        let socket_addr = addr.clone();
                        tokio::spawn(async move {
                            handler(shared_stream.clone(), socket_addr, message_tx_clone, close_tx_clone).await;
                        });
                    },
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
            }
            // クライアント切断時
            Ok(addr) = close_rx.recv() => {
                let mut new_clients = vec![];
                for client in clients {
                    let client_addr = client.lock().await.peer_addr().unwrap();
                    if client_addr != addr {
                        new_clients.push(client);
                    }
                }
                clients = new_clients;
            }
            // メッセージ受信時
            Ok(message) = message_rx.recv() => {
                for client in clients.iter_mut() {
                    if let Err(_) = client.lock().await.try_write(message.as_bytes()) {
                        continue;
                    }
                }
            }
        }
    }
}

async fn handler(stream: Arc<Mutex<TcpStream>>, addr: SocketAddr, message_tx: broadcast::Sender<String>, close_tx: broadcast::Sender<SocketAddr>) {
    let mut buffer = vec![];

    let socket_addr = addr.clone();

    let message_tx_clone = message_tx.clone();
    let close = move || {
        message_tx_clone.send(format!("{} is disconnected.\n", addr)).unwrap();
        close_tx.send(socket_addr).unwrap();
    };

    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(600), stream.lock().await.read(buffer.as_mut_slice())).await {
            Ok(Err(_)) | Err(_) => {
                close();
                break;
            },
            Ok(Ok(0)) => {
                close();
                break;
            },
            Ok(Ok(n)) => {
                let message_tx_clone = Sender::clone(&message_tx);
                let socket_addr = addr.clone();
                let message = String::from_utf8_lossy(&buffer[..n]);
                message_tx_clone.send(format!("{} : {}", socket_addr, message)).unwrap();
            }
        }

        buffer.clear();
    }
}
