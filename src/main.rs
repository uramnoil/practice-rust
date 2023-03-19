use std::{net::{TcpListener, TcpStream, SocketAddr}, thread, io::{Read, Write}, sync::{Arc, Mutex}};
use crossbeam_channel::{unbounded, Sender};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").expect("Failed to bind.");
    let (message_tx, message_rx) = unbounded::<String>();
    let (close_tx, close_rx) = unbounded::<SocketAddr>();
    let clients: Arc<Mutex<Vec<TcpStream>>> = Arc::new(Mutex::new(Vec::new()));

    let clients_clone = Arc::clone(&clients);
    // clientのクローズを待機
    thread::spawn(move || {
        for addr in close_rx {
            let index = clients_clone.lock().unwrap().iter().position(|x| x.peer_addr().unwrap() == addr).unwrap();
            clients_clone.lock().unwrap().remove(index);
        }
    });
    
    let clinets_clone = Arc::clone(&clients);
    // メッセージの配信
    thread::spawn(move || {
        loop {
            let message = message_rx.recv().unwrap();
            for mut client in clinets_clone.lock().unwrap().iter() {
                client.write_all(message.as_bytes()).unwrap();
            }
        }
    });

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                clients.lock().unwrap().push(stream.try_clone().unwrap());
                let message_tx_clone = message_tx.clone();
                let close_tx_clone = close_tx.clone();
                thread::spawn(move || {
                    handler(stream, message_tx_clone.clone(), close_tx_clone.clone());
                });
            },
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
}

fn handler(mut stream: TcpStream, message_tx: Sender<String>, close_tx: Sender<SocketAddr>) {
    let mut buffer = vec![];

    let socket_addr = stream.peer_addr().unwrap().clone();
    let close = move || {
        close_tx.send(socket_addr).unwrap();
    };

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                close();
                break;
            },
            Ok(n) => {
                let message = String::from_utf8_lossy(&buffer[..n]);
                message_tx.send(message.into_owned()).unwrap();
            },
            Err(e) => {
                println!("Error: {}", e);
                close();
                break;
            }
        }
    }
}