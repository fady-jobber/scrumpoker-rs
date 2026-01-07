mod models;

use futures::{SinkExt, StreamExt};
use models::{ClientMessage, Room, Rooms, ServerMessage, User};
use rocket::fs::{FileServer, Options, relative};
use rocket::serde::json::Json;
use rocket::{State, get, launch, post, routes};
use rocket_dyn_templates::{Template, context};
use rocket_ws::{Channel, WebSocket};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[get("/")]
async fn root() -> Template {
    Template::render("root", context! {})
}

#[get("/session/<room_id>")]
async fn session(room_id: String, rooms: &State<Rooms>) -> Template {
    let rooms_lock = rooms.read().await;
    if rooms_lock.contains_key(&room_id) {
        Template::render("session", context! { room_id: room_id })
    } else {
        Template::render("error", context! { message: "Room not found" })
    }
}

#[post("/api/create_room")]
async fn create_room(rooms: &State<Rooms>) -> Json<String> {
    let mut rooms_lock = rooms.write().await;
    let room = Room::new();
    let room_id = room.id.clone();
    rooms_lock.insert(room_id.clone(), room);
    Json(room_id)
}

#[get("/ws")]
fn ws(ws: WebSocket, rooms: &State<Rooms>) -> Channel<'static> {
    let rooms = rooms.inner().clone();
    ws.channel(move |stream| Box::pin(handle_websocket(stream, rooms)))
}

async fn handle_websocket(
    stream: rocket_ws::stream::DuplexStream,
    rooms: Rooms,
) -> Result<(), rocket_ws::result::Error> {
    let (mut sink, mut stream) = stream.split();
    let mut current_room_id: Option<String> = None;
    let mut broadcast_rx: Option<tokio::sync::broadcast::Receiver<String>> = None;

    loop {
        tokio::select! {
            msg = stream.next() => {
                if !handle_client_message(msg, &mut sink, &rooms, &mut current_room_id, &mut broadcast_rx).await {
                    break;
                }
            }
            broadcast_msg = receive_broadcast(&mut broadcast_rx) => {
                if let Ok(msg) = broadcast_msg {
                    let _ = sink.send(rocket_ws::Message::Text(msg)).await;
                }
            }
        }
    }
    Ok(())
}

async fn handle_client_message(
    msg: Option<Result<rocket_ws::Message, rocket_ws::result::Error>>,
    sink: &mut futures::stream::SplitSink<rocket_ws::stream::DuplexStream, rocket_ws::Message>,
    rooms: &Rooms,
    current_room_id: &mut Option<String>,
    broadcast_rx: &mut Option<tokio::sync::broadcast::Receiver<String>>,
) -> bool {
    match msg {
        Some(Ok(rocket_ws::Message::Text(text))) => {
            let client_msg = match serde_json::from_str::<ClientMessage>(&text) {
                Ok(msg) => msg,
                Err(_) => return true,
            };

            match &client_msg {
                ClientMessage::Join { ref room_id, .. } | ClientMessage::Rejoin { ref room_id, .. } => {
                    *current_room_id = Some(room_id.clone());
                    subscribe_to_room(rooms, room_id, broadcast_rx).await;
                }
                _ => {}
            }

            process_message(client_msg, rooms, current_room_id, sink).await;
            true
        }
        Some(Ok(rocket_ws::Message::Close(_))) | None => false,
        _ => true,
    }
}

async fn subscribe_to_room(
    rooms: &Rooms,
    room_id: &str,
    broadcast_rx: &mut Option<tokio::sync::broadcast::Receiver<String>>,
) {
    let rooms_lock = rooms.read().await;
    if let Some(room) = rooms_lock.get(room_id) {
        if let Some(ref tx) = room.broadcast_tx {
            *broadcast_rx = Some(tx.subscribe());
        }
    }
}

async fn process_message(
    client_msg: ClientMessage,
    rooms: &Rooms,
    current_room_id: &Option<String>,
    sink: &mut futures::stream::SplitSink<rocket_ws::stream::DuplexStream, rocket_ws::Message>,
) {
    match handle_message(client_msg, rooms).await {
        Ok(response) => {
            send_response(response.clone(), sink).await;

            if let Some(room_id) = current_room_id {
                if matches!(response, ServerMessage::Joined { .. }) {
                    broadcast_room_state(rooms, room_id).await;
                } else {
                    broadcast_to_room(rooms, room_id, response).await;
                }
            }
        }
        Err(e) => {
            let error = ServerMessage::Error { message: e };
            send_response(error, sink).await;
        }
    }
}

async fn send_response(
    response: ServerMessage,
    sink: &mut futures::stream::SplitSink<rocket_ws::stream::DuplexStream, rocket_ws::Message>,
) {
    if let Ok(json) = serde_json::to_string(&response) {
        let _ = sink.send(rocket_ws::Message::Text(json)).await;
    }
}

async fn broadcast_room_state(rooms: &Rooms, room_id: &str) {
    let rooms_lock = rooms.read().await;
    if let Some(room) = rooms_lock.get(room_id) {
        let room_state = ServerMessage::RoomState { room: room.clone() };
        broadcast_to_room(rooms, room_id, room_state).await;
    }
}

async fn receive_broadcast(
    rx: &mut Option<tokio::sync::broadcast::Receiver<String>>,
) -> Result<String, tokio::sync::broadcast::error::RecvError> {
    match rx {
        Some(receiver) => receiver.recv().await,
        None => std::future::pending().await,
    }
}

async fn broadcast_to_room(rooms: &Rooms, room_id: &str, message: ServerMessage) {
    let rooms_lock = rooms.read().await;
    if let Some(room) = rooms_lock.get(room_id) {
        if let Some(ref tx) = room.broadcast_tx {
            let json = serde_json::to_string(&message).unwrap();
            let _ = tx.send(json);
        }
    }
}

async fn handle_message(msg: ClientMessage, rooms: &Rooms) -> Result<ServerMessage, String> {
    match msg {
        ClientMessage::Join { room_id, name } => {
            let mut rooms_lock = rooms.write().await;
            let room = rooms_lock.get_mut(&room_id).ok_or("Room not found")?;

            let user_id = Uuid::new_v4().to_string();
            let user = User {
                id: user_id.clone(),
                name,
                estimate: None,
            };

            room.users.insert(user_id.clone(), user);

            Ok(ServerMessage::Joined { user_id, room_id })
        }
        ClientMessage::Rejoin { room_id, user_id, name } => {
            let mut rooms_lock = rooms.write().await;
            let room = rooms_lock.get_mut(&room_id).ok_or("Room not found")?;

            if let Some(user) = room.users.get_mut(&user_id) {
                user.name = name;
            } else {
                let user = User {
                    id: user_id.clone(),
                    name,
                    estimate: None,
                };
                room.users.insert(user_id.clone(), user);
            }

            Ok(ServerMessage::Joined { user_id, room_id })
        }
        ClientMessage::Vote {
            room_id,
            user_id,
            estimate,
        } => {
            let mut rooms_lock = rooms.write().await;
            let room = rooms_lock.get_mut(&room_id).ok_or("Room not found")?;

            if let Some(user) = room.users.get_mut(&user_id) {
                user.estimate = Some(estimate);
            }

            Ok(ServerMessage::RoomState { room: room.clone() })
        }
        ClientMessage::Show { room_id } => {
            let mut rooms_lock = rooms.write().await;
            let room = rooms_lock.get_mut(&room_id).ok_or("Room not found")?;

            room.revealed = true;

            Ok(ServerMessage::RoomState { room: room.clone() })
        }
        ClientMessage::Clear { room_id } => {
            let mut rooms_lock = rooms.write().await;
            let room = rooms_lock.get_mut(&room_id).ok_or("Room not found")?;

            for user in room.users.values_mut() {
                user.estimate = None;
            }
            room.revealed = false;

            Ok(ServerMessage::RoomState { room: room.clone() })
        }
    }
}

#[launch]
fn rocket() -> _ {
    let rooms: Rooms = Arc::new(RwLock::new(HashMap::new()));

    rocket::build()
        .attach(Template::fairing())
        .manage(rooms)
        .mount(
            "/public",
            FileServer::new(
                relative!("/public"),
                Options::Missing | Options::NormalizeDirs,
            ),
        )
        .mount("/", routes![root, session, create_room, ws])
}
