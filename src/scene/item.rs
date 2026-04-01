use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Item {
    pub name: String,
    pub description: String,
    pub tags: Vec<ItemTag>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemTag {
    Drink,
    Food,
    Weapon,
}

impl Display for ItemTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemTag::Drink => write!(f, "可飲用"),
            ItemTag::Food => write!(f, "可食用"),
            ItemTag::Weapon => write!(f, "武器"),
        }
    }
}
