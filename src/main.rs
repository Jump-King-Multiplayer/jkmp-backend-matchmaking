use futures::{SinkExt, StreamExt};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex},
};
use tokio_util::codec::{Decoder, Framed};

mod codec;
use codec::MessagesCodec;

mod messages;
use messages::Message;

type MessageType = Message;

struct Client {
    tx: mpsc::UnboundedSender<MessageType>,
    steam_id: u64,
    name: String,
}

impl Client {
    fn new(tx: mpsc::UnboundedSender<MessageType>) -> Self {
        Self {
            tx,
            steam_id: 0,
            name: String::default(),
        }
    }
}

struct State {
    clients: HashMap<SocketAddr, Client>,
}

impl State {
    fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let listener = TcpListener::bind("127.0.0.1:16000").await?;
    let state = Arc::new(Mutex::new(State::new()));

    loop {
        let result = listener.accept().await;

        match result {
            Err(error) => println!("An error occurred when accepting socket: {}", error),
            Ok((socket, address)) => {
                process_client(socket, address, state.clone()).await;
            }
        }
    }
}

async fn process_client(socket: TcpStream, address: SocketAddr, state: Arc<Mutex<State>>) {
    tokio::spawn(async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<MessageType>();
        let mut messages = MessagesCodec::new().framed(socket);

        match messages.next().await {
            Some(Ok(message)) => match message {
                Message::HandshakeRequest { steam_id } => {
                    let client = Client::new(tx);
                    state.lock().await.clients.insert(address, client);
                }
                _ => {
                    println!("Invalid handshake received from {}", address);
                    return;
                }
            },
            Some(Err(error)) => {
                println!(
                    "Error occurred while reading handshake from {}: {:?}",
                    address, error
                );
                return;
            }
            _ => {
                return;
            }
        }

        loop {
            tokio::select! {
                Some(outbound_message) = rx.recv() => {
                    if let Err(error) = messages.send(outbound_message).await {
                        println!("Failed to send message to {}: {:?}", address, error);
                        break; // Client disconnected
                    }
                },
                result = messages.next() => match result {
                    Some(Ok(message)) => {
                        if let Err(error) = handle_message(&mut messages, message, &address, &state).await {
                            println!("An error occured when handling message from {}: {:?}", address, error);
                            break;
                        }
                    },
                    Some(Err(error)) => {
                        println!("An error occured when reading message from {}: {:?}", address, error);
                    },
                    None => break // Client disconnected
                },
                else => break // Client disconnected
            }
        }

        state.lock().await.clients.remove(&address);
    });
}

async fn handle_message(
    messages: &mut Framed<TcpStream, MessagesCodec>,
    message: Message,
    address: &SocketAddr,
    state: &Arc<Mutex<State>>,
) -> Result<(), anyhow::Error> {
    println!("handling message: {:?}", message);
    Ok(())
}
