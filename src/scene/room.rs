use std::fmt::Debug;

use rig::message::Message;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::ai::actor::Actor;

use super::item::Item;

#[derive(Clone, Deserialize, Serialize)]
pub struct Room {
    /// The name of this room
    pub name: String,
    /// A brief description of this room
    pub description: String,
    /// A brief description of this room
    pub items: Vec<Item>,
    /// A list of actors currently in this room
    #[serde(skip)]
    pub actors: Vec<Actor>,
    /// Broadcast a message to all actors in this room
    #[serde(skip, default = "default_broadcast")]
    pub broadcast: broadcast::Sender<Message>,
}

impl Room {
    pub fn display_actors(&self) -> Vec<String> {
        self.actors
            .iter()
            .map(|actor| {
                let name = actor.name();
                let status = actor.status();
                if status.is_empty() {
                    name.clone()
                } else {
                    format!(
                        "{} ({})",
                        name,
                        status
                            .iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            })
            .collect()
    }

    pub fn context(&self) -> String {
        let actors = self.display_actors().join("\n");
        format!(
            "[區域]
你現在在 {} ，你只能與當前區域的東西互動。
目前在場的人有:
{}",
            self.name, actors
        )
    }

    pub async fn enter(&mut self, actor: Actor) {
        actor
            .agent()
            .lock()
            .await
            .broadcast
            .insert("room", self.broadcast.subscribe());
        self.actors.push(actor);
    }

    pub async fn exit(&mut self, actor: &Actor) {
        actor
            .agent()
            .lock()
            .await
            .broadcast
            .remove("room");
        self.actors.retain(|a| a != actor);
    }
}

impl Debug for Room {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Room {{ name: {}, description: {}, actors: {:?} }}", self.name, self.description, self.actors)
    }
}

fn default_broadcast() -> broadcast::Sender<Message> {
    broadcast::Sender::new(128)
}
