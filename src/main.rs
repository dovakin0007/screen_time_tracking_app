use std::{time, collections::HashMap};
use std::time::Instant;

use chrono::Local;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast::{ Sender }, Mutex,broadcast};
use tokio::net::TcpListener;
use futures_util::{future, pin_mut, StreamExt, TryStreamExt};
use futures_channel::mpsc::{unbounded, UnboundedSender};

use tokio_tungstenite::tungstenite::Message;

mod app_data;
mod app;
mod win;

use app_data::*;
use win::*;
type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;


pub async fn app(app_spent_time_map: Arc<Mutex<HashMap<String, AppData>>>, tx_t: Sender<String>) {
    let start = Instant::now();


    let mut app_spent_time_map = (app_spent_time_map.lock()).await;
    // let mut app_spent_time_map = *(app_spent_time_map_new.clone());

    let mut last_val = get_title_vec();

    let dt1= Local::now();
    let today = dt1.date_naive();
    
    let current_date = today.to_string();

    let idle_time = get_last_input_info().unwrap().as_secs();

    if idle_time >= 300 {
        last_val = "Idle Time".parse().unwrap()
    }    

    if app_spent_time_map.contains_key(last_val.as_str()) {
        app_spent_time_map.get_mut(last_val.as_str()).unwrap().update_seconds(1) ;
    }else {
       let mut main_key = last_val.clone().to_owned(); 
       main_key.push_str(&current_date);
        app_spent_time_map.insert(last_val.to_owned(), AppData::new(last_val.clone().to_owned(), 1, current_date.clone(), main_key));
    }

    let (_key, value) = &app_spent_time_map.get_key_value(last_val.as_str()).unwrap();
    let ws_string = value.get_string();

    tx_t.send(ws_string).expect("Unable to send data");
//    println!("{key}: {value:?}");

    if app_spent_time_map.get(&*last_val).unwrap().get_date().to_string() != current_date.clone(){
        // println!("got called");
        app_spent_time_map.get_mut(&*last_val).unwrap().reset_time(current_date.clone())
    }

    let duration = start.elapsed();

    update_db(&app_spent_time_map).await.unwrap();
   println!("Time elapsed in expensive_function() is: {:?}", duration);
    let time_delay_for_function = 1000 - duration.as_millis();
    let delay = time::Duration::from_millis(time_delay_for_function.try_into().unwrap_or(1000));
    tokio::time::sleep(delay).await;
}
#[tokio::main]
async fn main(){
    let (tx_t_broadcast, _rx_t_broadcast) = broadcast::channel(1024);
    let listener = TcpListener::bind("127.0.0.1:8080").await.expect("Unable to bind to server");
    let state = PeerMap::new(Mutex::new(HashMap::new()));

    let mut app_time_spent_map = HashMap::new();
    let current_day= Local::now();
    let today_date = current_day.date_naive();
    let app_spent_time_new: &mut HashMap<String, AppData>=get_data_from_db(&mut app_time_spent_map, &today_date).await.unwrap();
    let app_spent_time:Arc<Mutex<HashMap<String, AppData>>> = Arc::new(Mutex::new(app_spent_time_new.to_owned()));

    // let ws_app_spent_time = app_spent_time.clone();
    // let mut rx_t_broadcast_clone = tx_t_broadcast.subscribe();
    let tx_t_broadcast_new = tx_t_broadcast.clone();
    tokio::spawn(async move{

        while let Ok((stream, addr)) = listener.accept().await {
            let tx_t_broadcast = tx_t_broadcast.clone();
            let state = state.clone();
            tokio::spawn(async move{
                let tx_t_broadcast = tx_t_broadcast.clone();
                println!("Connection from: {}", addr);
                let ws_stream = tokio_tungstenite::accept_async(stream).await.expect("Error during the websocket handshake occurred");
                println!("WebSocket connection established: {}", addr);

                let (tx, rx) = unbounded();
                (state.lock()).await.insert(addr, tx);
                let new_state = state.clone();
                let (outgoing, incoming) = ws_stream.split();
                let broadcast_incoming = incoming.try_for_each( move|msg| {
                    let new_state = Arc::clone(&new_state);
                    let mut rx_t_new_clone = tx_t_broadcast.subscribe();
                    async move {
                        println!("Received a message from {}: {}", addr, msg.to_text().unwrap());

                        let peers = new_state.lock().await;

                        let broadcast_recipients = peers.iter().map(|(_, ws_sink)| ws_sink);

                        let received_data = if let Ok(recv_data) = rx_t_new_clone.recv().await {
                            println!("{recv_data}");
                            recv_data
                        }else {
                            println!("No data received");
                            String::from("Error couldnt read the data")
                        };
                        // println!("{}",rx_t_new_clone.recv().await.unwrap());

                        for recp in broadcast_recipients {
                            recp.unbounded_send(msg.clone()).expect("unable to send the message");
                            recp.unbounded_send(Message::Binary(received_data.clone().into_bytes())).expect("unable to parse and send the message");
                        }
                        Ok(())
                    }
                });

                let receive_from_others = rx.map(Ok).forward(outgoing);
                pin_mut!(broadcast_incoming, receive_from_others);
                future::select(broadcast_incoming, receive_from_others).await;


                println!("{} disconnected", &addr);
                state.lock().await.remove(&addr);
            });
        }
    });
    loop{

        let app_spent_time= app_spent_time.clone();
        tokio::spawn(app(app_spent_time, tx_t_broadcast_new.clone())).await.unwrap();


    }

}
    