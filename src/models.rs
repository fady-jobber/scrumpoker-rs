use rand::Rng;
use rocket::serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub estimate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub users: HashMap<String, User>,
    pub revealed: bool,
    #[serde(skip)]
    pub broadcast_tx: Option<broadcast::Sender<String>>,
}

impl Room {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        let mut rng = rand::thread_rng();
        let id = format!("{:03}", rng.gen_range(100..1000));
        Room {
            id,
            users: HashMap::new(),
            revealed: false,
            broadcast_tx: Some(tx),
        }
    }
}

pub type Rooms = Arc<RwLock<HashMap<String, Room>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    Join {
        room_id: String,
        name: String,
    },
    Rejoin {
        room_id: String,
        user_id: String,
        name: String,
    },
    Vote {
        room_id: String,
        user_id: String,
        estimate: String,
    },
    Show {
        room_id: String,
    },
    Clear {
        room_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    RoomState { room: Room },
    Error { message: String },
    Joined { user_id: String, room_id: String },
}
