use std::{cell::RefCell, collections::BTreeMap, fmt::Display};

use anyhow::{Result, anyhow};
use room::Room;

use crate::ai::{actor::{Actor, ActorAgent}, director::Director};

mod build;
pub mod room;

#[derive(Debug)]
pub struct Scene {
    pub description: String,
    pub rooms: BTreeMap<String, Room>,
}

impl Scene {
    pub async fn generate(director: &Director) -> Result<Self> {
        build::generate(director).await
    }
    pub fn context(&self) -> String {
        let rooms = self
            .rooms
            .values()
            .map(|room| format!("{}: {}", room.name, room.description))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "[場景]
你現在在一個場景中，場景的描述如下:
{}
場景包含以下區域:
{}",
            self.description, rooms
        )
    }
    /// flexible match room name, just check if the room name contains the input name
    pub fn match_room(&self, name: &str) -> Option<&String> {
        self.rooms.keys().find(|room_name| name.contains(*room_name))
    }
    pub fn get_room_by_actor(&self, actor: &Actor) -> Option<&Room> {
        self.rooms.values().find(|room| room.actors.contains(actor))
    }
    pub fn get_room_by_actor_mut(&mut self, actor: &Actor) -> Option<&mut Room> {
        self.rooms.values_mut().find(|room| room.actors.contains(actor))
    }
    pub async fn swap_actor_room(&mut self, actor: &Actor, from: &String, to: &String) -> Result<()> {
        let from_room = self.rooms.get_mut(from).ok_or(anyhow!("Room {} not found", from))?;
        let actor = from_room.actors.swap_remove(from_room.actors.iter().position(|a| a == actor).unwrap());
        let to_room = self.rooms.get_mut(to).ok_or(anyhow!("Room {} not found", to))?;
        to_room.enter(actor).await;
        Ok(())
    }
}

impl Display for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}\n", self.description)?;
        for (name, room) in &self.rooms {
            writeln!(f, "{}: {}", name, room.description)?;
        }
        Ok(())
    }
}
