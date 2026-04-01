#![feature(iterator_try_collect)]

use std::{fmt::Write, time::Duration};

use ai::{
    actor::{Actor, ActorStatus},
    director::Director,
};
use anyhow::{Result, bail};
use config::Config;
use dialoguer::{Confirm, Input};
use rig::message::Message;
use role::Role;
use scene::Scene;
use tokio::{select, sync::Mutex, time::sleep};
use tracing::{error, warn};
use utils::Pending;

pub mod ai;
pub mod config;
pub mod role;
pub mod scene;
pub mod utils;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let config = Config::init().expect("Failed to load config");
    let mut game = Game::new(config).await.expect("Failed to create game");

    println!("區域列表:");
    for room in game.scene.lock().await.rooms.values() {
        println!("{}: {}", room.name, room.description);
    }

    println!("\n玩家列表:");
    let mut record = String::new();
    for actor in game.actors.iter_mut() {
        println!("{}: {}", actor.name(), actor.role());
        writeln!(record, "{}: {}", actor.name(), actor.role()).unwrap();
    }

    for actor in game
        .actors
        .iter()
        .filter(|a| matches!(a.role(), Role::Sheriff))
    {
        let mut agent = actor.agent().lock().await;
        agent
            .history
            .push(Message::user(format!("小提示: \n{}", record)));
    }

    println!("\n---遊戲開始---");

    loop {
        game.action().await.unwrap();

        if Confirm::new()
            .with_prompt("顯示首位玩家的提示詞")
            .interact()
            .expect("Failed to read input")
        {
            let actor = game.actors.first().unwrap();
            game.sync_actor(actor).await.unwrap();
            let agent = actor.agent().lock().await;
            println!("---{} 的提示詞---", actor.name());
            println!("{}", agent.agent.preamble.as_ref().unwrap());

            println!("---{} 的對話紀錄---", actor.name());
            for message in agent.history.iter() {
                println!("{:?}", message);
            }
        }

        println!("---< 時間推進了5分鐘... >---\n");
        sleep(Duration::from_secs(1)).await;
    }
}

pub struct Game {
    director: Director,
    scene: Mutex<Scene>,
    actors: Vec<Actor>,
    /// start at 0:00
    /// ecah value is 5 minutes, so 288 values for 24 hours
    time: u32,

    config: Config,
}

impl Game {
    pub async fn new(config: Config) -> Result<Self> {
        let agent = config.agent().expect("Failed to create agent");

        let director = pending_until!(Director::new(agent), ["尋找敘事者", "評估敘事者",]);

        let mut scene = pending_until!(
            Scene::generate(&director),
            ["生成場景概念", "生成區域描述", "設計場景結構"]
        )
        .unwrap();

        let actors_len: u8 = Input::new()
            .with_prompt("遊玩人數")
            .default(5)
            .interact()
            .unwrap();
        let actors = pending_until!(
            Actor::generate_many(&director, actors_len),
            ["尋找玩家", "檢查性格",]
        )
        .unwrap();

        let scene_context = scene.context();
        for actor in actors.iter().cloned() {
            // load scene context into each
            actor
                .agent()
                .lock()
                .await
                .context
                .insert("scene", scene_context.clone());
            // insert into the first room
            scene
                .rooms
                .first_entry()
                .unwrap()
                .get_mut()
                .enter(actor)
                .await;
        }

        Ok(Game {
            director,
            scene: Mutex::new(scene),
            actors,
            config,
            time: 0,
        })
    }

    pub async fn action(&mut self) -> Result<()> {
        if self.check_finished() {
            bail!("Game finished");
        }

        for actor in self.actors.clone() {
            if actor.status().contains(&ActorStatus::Dead) {
                continue;
            };

            self.sync_actor(&actor).await.unwrap();
            self.action_actor(&actor).await.unwrap();
            println!();
        }

        self.time += 1;
        Ok(())
    }

    pub fn check_finished(&self) -> bool {
        // check time
        if self.time >= 288 {
            println!("---一天結束了，遊戲結束了---");
            return true;
        }

        // check win
        let (alive_killers, alive_innocents): (Vec<_>, Vec<_>) = self
            .actors
            .iter()
            .filter(|a| !a.status().contains(&ActorStatus::Dead))
            .partition(|a| matches!(a.role(), Role::Murderer));

        alive_killers.is_empty() || alive_innocents.is_empty()
    }

    pub async fn sync_actor(&self, actor: &Actor) -> Result<()> {
        let mut agent = actor.agent().lock().await;

        // refresh room context
        let scene = self.scene.lock().await;
        let room = scene.get_room_by_actor(actor).unwrap();
        agent.context.insert("room", room.context());

        // sync all
        agent.sync()
    }

    pub async fn action_actor(&mut self, actor: &Actor) -> Result<()> {
        let mut scene = self.scene.lock().await;

        /// (action name, action content, action description, action thinking)
        type ActionPrompt = (&'static str, &'static str, &'static str, &'static str);
        let mut prompt: Vec<ActionPrompt> = vec![
            (
                "SILENT",
                "",
                "你選擇保持沉默，不說話。",
                "我想從他們的對話中取得更多信息。",
            ),
            (
                "TALK",
                "對話內容 (不需要加上引號)",
                "你嘗試與其他玩家進行對話，分享你的想法或詢問他們的狀況。",
                "我想通過對話來獲取更多信息，或者迷惑其他玩家讓他們暴露身份。",
            ),
            (
                "GOTO",
                "地點",
                "你決定前往其他區域，尋找新的線索或避開危險。",
                "我認為我在這裡待著不安全，或者這裡沒有什麼有用的信息了，我想去其他地方看看。",
            ),
            (
                "OBSERVE",
                "",
                "你仔細觀察周圍的環境，尋找可能被忽略的線索。",
                "我想看看周圍有沒有什麼我之前沒有注意到的細節，或者確認一下這裡的環境狀況。",
            ),
        ];

        if matches!(actor.role(), Role::Murderer | Role::Sheriff) {
            prompt.push((
                "ATTACK",
                "玩家名稱",
                "你選擇殺死一名玩家。",
                "我有足夠的信心能確定他是敵人。",
            ));
        }

        let mut flatten_prompt = "## 以下是可用的行為列表\n".to_string();
        for (i, (action, content, description, thinking)) in prompt.into_iter().enumerate() {
            flatten_prompt += &format!(
                "{}. {}:
```json
{{
  \"thought\": \"{}\",
  \"action\": \"{}\",
  \"content\": \"{}\",
}}
```\n\n",
                i + 1,
                description,
                thinking,
                action,
                content
            );
        }

        // do action
        for _attempt in 1..=5 {
            let Ok(action) = async {
                let mut agent = actor.agent().lock().await;
                agent.action(&flatten_prompt).await
            }
            .await
            else {
                continue;
            };

            macro_rules! handle_invalid_action {
                ($($arg:tt)*) => {
                    warn!("{}: {}", actor.name(), format!($($arg)*));
                    actor
                        .agent()
                        .lock()
                        .await
                        .history
                        .push(Message::user(format!($($arg)*)));
                }
            }

            macro_rules! broadcast {
                ($broadcast: expr, $($arg:tt)*) => {
                    println!($($arg)*);
                    $broadcast.broadcast.send(Message::user(format!($($arg)*))).unwrap();
                }
            }

            match action.action.as_str() {
                "SILENT" => {
                    println!("< {} 選擇沉默 >", actor.name());
                    return Ok(());
                }
                "TALK" => {
                    let room = scene.get_room_by_actor(actor).unwrap();
                    broadcast!(room, "{}: {}", actor.name(), action.content);
                    return Ok(());
                }
                "GOTO" => {
                    let from_room = scene.get_room_by_actor(actor).unwrap();
                    let from = from_room.name.clone();
                    let Some(to) = scene.match_room(&action.content).cloned() else {
                        handle_invalid_action!(
                            "你嘗試前往 {}, 但是這個區域不存在，請重新選擇一個區域。",
                            action.content
                        );
                        continue;
                    };

                    if from == to {
                        handle_invalid_action!(
                            "你嘗試前往 {}, 但是你已經在這個區域了，請重新選擇一個區域。",
                            action.content
                        );
                        continue;
                    }

                    broadcast!(from_room, "< {} 離開 {}，前往 {} >", actor.name(), from, to);
                    scene.swap_actor_room(actor, &from, &to).await?;
                    return Ok(());
                }
                "OBSERVE" => {
                    println!("< {} 仔細觀察周圍的環境 >", actor.name());

                    actor.agent().lock().await.history.push(Message::user(
                        "你仔細觀察周圍的環境發現: 大量血跡".to_string(),
                    ));

                    return Ok(());
                }
                "ATTACK" => {
                    let room = scene.get_room_by_actor(actor).unwrap();
                    for target in room.actors.iter() {
                        if target.name() == &action.content {
                            if actor == target {
                                handle_invalid_action!(
                                    "你嘗試攻擊 {}, 但是你不能攻擊自己，請重新選擇一個行動。",
                                    action.content
                                );
                                continue;
                            }

                            if target.status().contains(&ActorStatus::Dead) {
                                handle_invalid_action!(
                                    "你嘗試攻擊 {}, 但是他已經死了，請重新選擇一個行動。",
                                    action.content
                                );
                                continue;
                            }

                            broadcast!(
                                room,
                                "< {} 使用 {} 殺死了 {} >",
                                actor.name(),
                                match actor.role() {
                                    Role::Sheriff => "手槍",
                                    Role::Murderer => "匕首",
                                    Role::Innocent => "空手",
                                },
                                target.name()
                            );

                            let status = target.status();
                            status.insert(ActorStatus::Dead);

                            target.agent().lock().await.broadcast.clear();
                            return Ok(());
                        }
                    }

                    handle_invalid_action!(
                        "你嘗試攻擊不在此區域的人 {}, 可能在其他區域或是錯字。",
                        action.content
                    );
                }
                name => {
                    error!("Unexpect action name: {}", name);
                }
            };
        }

        bail!("failed to action: {:?}", actor);
    }
}
