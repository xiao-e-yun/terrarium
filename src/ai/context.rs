use std::{collections::BTreeMap, fmt::Display};

use crate::config::TAgent;

pub struct Context {
    pub base: String,
    pub additional: BTreeMap<&'static str, String>,
}

impl Context {
    pub fn new(base: String) -> Self {
        Self {
            base,
            additional: BTreeMap::new(),
        }
    }
    
    pub fn bind(&self, agent: &mut TAgent) {
        agent.preamble = Some(self.to_string());
    }

    pub fn insert(&mut self, key: &'static str, value: String) {
        self.additional.insert(key, value);
    }
}

impl Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.base)?;

        for value in self.additional.values() {
            writeln!(f, "\n{}", value)?;
        }

        Ok(())
    }
}
