use std::{
    io::{self, Write},
    mem,
    rc::Rc,
};

use ai::actor::Actor;
use config::Config;
use tokio::select;

pub mod ai;
pub mod config;
pub mod utils;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let config = Config::init().expect("Failed to load config");
    let agent = config.agent().expect("Failed to create agent");

    let mut act1 = Actor::new("大壯".to_string(), agent.clone());
    let mut act2 = Actor::new("小白".to_string(), agent);

    let pending = utils::Pending::default();
    let stdin = io::stdin();
    let mut ask = String::new();

    println!("辯論題目:");
    let mut history = vec![];
    stdin.read_line(&mut ask).unwrap();
    history.push(format!("辯論題目: {}", ask.trim()));

    let mut act = &mut act1;
    let mut act_other = &mut act2;
    loop {
        select! {
            _ = async {
                let content = act.speak(&history.join("\n")).await.unwrap();
                history.push(format!("{}: {}", act.name, content));

                let color = if act.name == "大壯" { "\x1B[34m" } else { "\x1B[31m" };
                let clear_color = "\x1B[0m";
                println!("\r\x1B[2K{}{}{}", color, content, clear_color);
                mem::swap(&mut act, &mut act_other);
            } => {}
            _ = async {
                loop {
                    print!("\r\x1B[2K{}", pending);
                    io::stdout().flush().unwrap();
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            } => {}
        }
    }
}
