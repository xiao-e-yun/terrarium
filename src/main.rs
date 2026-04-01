#![feature(iterator_try_collect)]

use std::{fmt::Write, time::Duration};

use ai::{
    actor::{Actor, ActorAttr, ActorStatus},
    director::Director,
};
use anyhow::{Result, bail};
use config::Config;
use dialoguer::{Confirm, Input};
use rand::seq::{IteratorRandom, SliceRandom};
use rig::message::{AssistantContent, Message, UserContent};
use role::Role;
use scene::{Scene, item::ItemTag};
use tokio::{select, sync::Mutex, time::sleep};
use tracing::warn;
use utils::Pending;

pub mod ai;
pub mod config;
pub mod role;
pub mod scene;
pub mod utils;

const ACTION_RETRY_LIMIT: usize = 32;

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

    println!("\n---遊戲開始---");
    loop {
        println!("{}", game.display_scene().await);

        if let Err(e) = game.action().await {
            println!("{:?}", e);
            break;
        };
        println!("---< 時間推進了5分鐘... >---\n");
        sleep(Duration::from_secs(1)).await;
    }

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
            let content = match message {
                Message::System { content } => content,
                Message::User { content } => &content
                    .iter()
                    .map(|c| match c {
                        UserContent::Text(text) => text.text.clone(),
                        _ => unreachable!(),
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                Message::Assistant { content, .. } => &content
                    .iter()
                    .map(|c| match c {
                        AssistantContent::Text(t) => t.text.clone(),
                        _ => unreachable!(),
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            };
            println!("{}", content);
        }
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
            let mut rng = rand::rng();
            let room = scene.rooms.values_mut().choose(&mut rng).unwrap();
            room.enter(actor).await;
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
            bail!("Game Over");
        }

        // random sort player action step
        let mut rng = rand::rng();
        let mut actors = self.actors.clone();
        actors.shuffle(&mut rng);

        // run action
        for actor in actors {
            if actor.status().contains(&ActorStatus::Dead) {
                continue;
            };

            for mut attr in actor.attrs().iter_mut() {
                *attr = attr.saturating_sub(1);
                if attr.value() == &0 {
                    println!("{}: {}死了", actor.name(), attr.key());
                    let scene = self.scene.lock().await;
                    let room = scene.get_room_by_actor(&actor).unwrap();
                    room.broadcast
                        .send(Message::user(format!(
                            "{}: {}死了",
                            actor.name(),
                            attr.key()
                        )))
                        .unwrap();
                }
            }

            self.sync_actor(&actor).await?;
            self.action_actor(&actor).await?;
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

        // refresh time
        let time = format!("{}:{}", self.time / 12, self.time % 12 * 5);
        agent
            .context
            .insert("time", format!("[時間]\n現在時間 {}", time));

        // refresh attrs & status
        let attrs = actor
            .attrs()
            .iter()
            .map(|attr| attr.key().display_attrs(*attr.value()))
            .collect::<Vec<_>>()
            .join("\n");
        agent.context.insert("attrs", format!("[狀態]\n{}", attrs));

        // refresh items list
        let items = actor
            .inventory()
            .iter()
            .map(|i| {
                let mut output = format!("{}: {}", i.name, i.description);
                let tags = i
                    .tags
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                if !tags.is_empty() {
                    output += &format!(" ({})", tags);
                };
                output
            })
            .collect::<Vec<_>>()
            .join("\n");
        agent.context.insert(
            "items",
            format!("[背包物品]\n以下道具是你藏在身上的物品\n{}", items),
        );

        // refresh room context
        let scene = self.scene.lock().await;
        let room = scene.get_room_by_actor(actor).unwrap();
        agent.context.insert("room", room.context());

        // sync all
        agent.sync()
    }

    pub async fn action_actor(&mut self, actor: &Actor) -> Result<()> {
        macro_rules! handle_invalid_action {
            ($actor:expr, $($arg:tt)*) => {
                warn!("{}: {}", $actor.name(), format!($($arg)*));
                $actor
                    .agent()
                    .lock()
                    .await
                    .history
                    .push(Message::user(format!("< {} >", format!($($arg)*))));
            }
        }

        // ── 對話 prompt (行動後共用) ─────────────────────────────────────────
        let talk_prompt = "## [對話階段]
現在是對話階段。請決定你是否要說話。

- 若想說話，將 `talk` 填入你的對話內容；只能包含單句話，不得換行。
- 若選擇沉默，將 `talk` 填入空字串 `\"\"`。

```json
{
  \"thought\": \"你的思考過程\",
  \"talk\": \"對話內容，或空字串\"
}
```
";

        // ── 行動 prompt ──────────────────────────────────────────────────────
        {
            type ActionPrompt = (&'static str, &'static str, &'static str, &'static str);
            let mut prompt: Vec<ActionPrompt> = vec![
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
                (
                    "PICKUP",
                    "物品名稱",
                    "你嘗試撿起一個物品，可能是武器、食物或飲料。",
                    "我認為這個物品對我有用，可能可以幫助我生存或者完成我的目標。",
                ),
                (
                    "IDLE",
                    "",
                    "你選擇不採取任何行動，靜觀其變。",
                    "現在沒有特別需要做的事，我決定先觀望一下情況。",
                ),
            ];

            if !actor.inventory().is_empty() {
                prompt.push((
                    "DROP",
                    "物品名稱",
                    "你選擇將身上的某個物品丟棄在原地，讓其他人可以拾取。",
                    "其他人目前比我更需要這個物品的幫助，所以我決定把它放下來提供給他們使用。",
                ));
            }

            if actor.inventory().iter().any(|item| {
                item.tags
                    .iter()
                    .any(|t| t == &ItemTag::Drink || t == &ItemTag::Food)
            }) {
                prompt.push((
                    "USE",
                    "物品名稱 (食物或飲料)",
                    "你選擇消耗一個物品（如吃下食物或喝下飲料）來恢復自身的狀態。",
                    "我目前的狀態需要補充體力或水分，必須藉由進食或飲水來確保接下來能夠繼續行動。",
                ));
            }

            if actor
                .inventory()
                .iter()
                .any(|item| item.tags.contains(&ItemTag::Weapon))
            {
                prompt.push((
                    "ATTACK",
                    "玩家名稱 (不包含對話)",
                    "你選擇殺死一名玩家，所有武器都是使用此行為。",
                    "我認為這名玩家對我構成了威脅，或者除掉他是我達成目標最快的方法。",
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
  \"talk\": \"執行此行動時想說的話，或空字串\"
}}
```\n\n",
                    i + 1,
                    description,
                    thinking,
                    action,
                    content
                );
            }

            // ── 執行行動 ─────────────────────────────────────────────────────
            'do_action: for _attempt in 0..ACTION_RETRY_LIMIT {
                let Ok(action) = async {
                    let mut agent = actor.agent().lock().await;
                    agent.action(&flatten_prompt).await
                }
                .await
                else {
                    continue;
                };

                macro_rules! broadcast {
                    ($broadcast: expr, $($arg:tt)*) => {
                        println!($($arg)*);
                        $broadcast.broadcast.send(Message::user(format!($($arg)*))).unwrap();
                    }
                }

                // 廣播 talk（行動同時說話）
                if !action.talk.is_empty() {
                    let scene = self.scene.lock().await;
                    let room = scene.get_room_by_actor(actor).unwrap();
                    println!("{}: {}", actor.name(), action.talk);
                    room.broadcast
                        .send(Message::user(format!("{}: {}", actor.name(), action.talk)))
                        .unwrap();
                }

                let mut scene = self.scene.lock().await;

                match action.action.as_str() {
                    "IDLE" => {
                        println!("< {} 靜觀其變 >", actor.name());
                        return Ok(())
                    }
                    "GOTO" => {
                        let from_room = scene.get_room_by_actor(actor).unwrap();
                        let from = from_room.name.clone();
                        let Some(to) = scene.match_room(&action.content).cloned() else {
                            let all_room = scene
                                .rooms
                                .keys()
                                .filter(|&r| r != &action.content)
                                .cloned()
                                .collect::<Vec<String>>()
                                .join("\n");
                            handle_invalid_action!(
                                actor,
                                "你嘗試前往 {}, 但是這個區域不存在，請重新選擇一個區域。\n{}",
                                action.content,
                                all_room
                            );
                            continue;
                        };

                        if from == to {
                            let all_room = scene
                                .rooms
                                .keys()
                                .filter(|&r| r != &to)
                                .cloned()
                                .collect::<Vec<String>>()
                                .join("\n");
                            handle_invalid_action!(
                                actor,
                                "你嘗試前往 {}, 但是你已經在這個區域了，請重新選擇一個區域。\n{}",
                                action.content,
                                all_room
                            );
                            continue;
                        }

                        broadcast!(from_room, "< {} 離開 {}，前往 {} >", actor.name(), from, to);
                        scene.swap_actor_room(actor, &from, &to).await?;
                        actor
                            .agent()
                            .lock()
                            .await
                            .history
                            .push(Message::user(format!("< 你前往{} >", to)));
                        break 'do_action;
                    }
                    "OBSERVE" => {
                        let room = scene.get_room_by_actor(actor).unwrap();
                        println!("< {} 仔細觀察周圍的環境 >", actor.name());
                        actor
                            .agent()
                            .lock()
                            .await
                            .history
                            .push(Message::user(format!(
                                "< 你仔細觀察 {} 周圍的環境 >\n你發現了以下物品:\n{}",
                                room.name,
                                room.items
                                    .iter()
                                    .map(|item| format!("{}: {}", item.name, item.description))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            )));
                        break 'do_action;
                    }
                    "PICKUP" => {
                        let room = scene.get_room_by_actor_mut(actor).unwrap();
                        let Some(item) = room
                            .items
                            .iter()
                            .position(|item| item.name == action.content)
                        else {
                            handle_invalid_action!(
                                actor,
                                "你嘗試撿起 {}, 但是這個物品不存在，請重新選擇一個物品，或是使用 OBSERVE 來確認一下周圍的物品。",
                                action.content
                            );
                            continue;
                        };
                        let item = room.items.swap_remove(item);

                        broadcast!(room, "< {} 撿起了 {} >", actor.name(), item.name);
                        actor.inventory().insert(item);
                        break 'do_action;
                    }
                    "DROP" => {
                        let item_opt = actor
                            .inventory()
                            .iter()
                            .find(|item| item.name == action.content)
                            .map(|ref_val| ref_val.clone());

                        let Some(item) = item_opt else {
                            handle_invalid_action!(
                                actor,
                                "你嘗試丟棄 {}, 但是你的背包裡沒有這個物品，請重新確認你身上的物品。",
                                action.content
                            );
                            continue;
                        };

                        actor.inventory().remove(&item);

                        let room = scene.get_room_by_actor_mut(actor).unwrap();
                        broadcast!(room, "< {} 將 {} 丟棄在原地 >", actor.name(), item.name);
                        room.items.push(item);
                        break 'do_action;
                    }
                    "USE" => {
                        let item_opt = actor
                            .inventory()
                            .iter()
                            .find(|item| item.name == action.content)
                            .map(|ref_val| ref_val.clone());

                        let Some(item) = item_opt else {
                            handle_invalid_action!(
                                actor,
                                "你嘗試使用 {}, 但是你的背包裡沒有這個物品，請重新確認你身上的物品。",
                                action.content
                            );
                            continue;
                        };

                        if !item.tags.contains(&ItemTag::Food) && !item.tags.contains(&ItemTag::Drink) {
                            handle_invalid_action!(
                                actor,
                                "你嘗試使用 {}, 但它不是食物或飲料，無法用來消耗與恢復狀態。",
                                action.content
                            );
                            continue;
                        }

                        actor.inventory().remove(&item);

                        let room = scene.get_room_by_actor(actor).unwrap();
                        broadcast!(
                            room,
                            "< {} 使用了 {}，恢復了自身的狀態 >",
                            actor.name(),
                            item.name
                        );

                        let attrs = actor.attrs();
                        for t in item.tags {
                            match t {
                                ItemTag::Food => *attrs.get_mut(&ActorAttr::Hunger).unwrap() += 128,
                                ItemTag::Drink => *attrs.get_mut(&ActorAttr::Thirst).unwrap() += 128,
                                ItemTag::Weapon => {}
                            }
                        }
                        break 'do_action;
                    }
                    "ATTACK" => {
                        let room = scene.get_room_by_actor_mut(actor).unwrap();
                        let mut attacked = false;
                        for target in room.actors.iter() {
                            if !action.content.contains(target.name()) {
                                continue;
                            }

                            if actor == target {
                                handle_invalid_action!(
                                    actor,
                                    "你嘗試攻擊 {}, 但是你不能攻擊自己，請重新選擇一個行動。",
                                    action.content
                                );
                                continue 'do_action;
                            }

                            if target.status().contains(&ActorStatus::Dead) {
                                handle_invalid_action!(
                                    actor,
                                    "你嘗試攻擊 {}, 但是他已經死了，請重新選擇一個行動。",
                                    action.content
                                );
                                continue 'do_action;
                            }

                            let weapon = actor
                                .inventory()
                                .iter()
                                .find(|i| i.tags.contains(&ItemTag::Weapon));

                            broadcast!(
                                room,
                                "< {} 使用 {} 殺死了 {} >",
                                actor.name(),
                                match &weapon {
                                    Some(v) => v.name.to_string(),
                                    None => "空手".to_string(),
                                },
                                target.name()
                            );

                            let status = target.status();
                            status.insert(ActorStatus::Dead);

                            let inventory = target.inventory();
                            let items: Vec<_> =
                                inventory.iter().map(|ref_val| ref_val.clone()).collect();
                            inventory.clear();

                            for item in items {
                                broadcast!(room, "< {} 掉落了 {} >", target.name(), item.name);
                                room.items.push(item);
                            }

                            attacked = true;
                            break;
                        }

                        if attacked {
                            break 'do_action;
                        }

                        handle_invalid_action!(
                            actor,
                            "你嘗試攻擊不在此區域的人 {}, 可能在其他區域。",
                            action.content
                        );
                    }
                    name => {
                        handle_invalid_action!(actor, "行為 {} 不存在，請使用列表中的行為", name);
                    }
                };
            }
        }

        // ── 行動後 sync ──────────────────────────────────────────────────────
        drop(self.scene.lock().await); // ensure no lock held
        self.sync_actor(actor).await?;

        // ── 執行對話 (後) ────────────────────────────────────────────────────
        for _attempt in 0..ACTION_RETRY_LIMIT {
            let Ok(result) = async {
                let mut agent = actor.agent().lock().await;
                agent.talk(talk_prompt).await
            }
            .await
            else {
                continue;
            };

            if result.talk.is_empty() {
                println!("< {} 選擇沉默 >", actor.name());
            } else {
                let scene = self.scene.lock().await;
                let room = scene.get_room_by_actor(actor).unwrap();
                println!("{}: {}", actor.name(), result.talk);
                room.broadcast
                    .send(Message::user(format!("{}: {}", actor.name(), result.talk)))
                    .unwrap();
            }
            break;
        }

        Ok(())
    }

    async fn display_scene(&self) -> String {
        let scene = self.scene.lock().await;
        let mut output = String::new();
        for (name, room) in scene.rooms.iter() {
            let actors = room
                .actors
                .iter()
                .filter(|a| !a.status().contains(&ActorStatus::Dead))
                .map(|a| a.name().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(output, "{}: {}", name, actors).unwrap();
        }
        output
    }
}
