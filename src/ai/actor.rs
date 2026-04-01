use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    sync::{Arc, atomic::AtomicU8},
};

use anyhow::{Result, bail};
use dashmap::{DashMap, DashSet};
use rand::seq::{IndexedRandom, SliceRandom};
use rig::{
    completion::{Chat, Prompt},
    message::Message,
};
use serde::Deserialize;
use tokio::sync::{Mutex, broadcast};
use tracing::{error, warn};

use crate::{
    config::TAgent,
    role::Role,
    scene::item::{Item, ItemTag},
};

use super::{context::Context, director::Director};

#[derive(Clone)]
pub struct Actor {
    // Readonly fields
    role: Role,
    name: String,

    personality: String,

    shared: Arc<ActorShared>,
}

struct ActorShared {
    attributes: DashMap<ActorAttr, u32>,
    status: DashSet<ActorStatus>,
    inventory: DashSet<Item>,
    agent: Mutex<ActorAgent>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ActorAttr {
    Hunger,
    Thirst,
}

impl ActorAttr {
    pub fn display_attrs(&self, value: u32) -> String {
        // 0~100
        match value {
            0..=5 => format!("你感到極度{}，快要撐不下去了.", self),
            6..=15 => format!("你感到非常{}，已經很難受了.", self),
            16..=25 => format!("你感到{}了，需要注意了.", self),
            26..=30 => format!("你感到{}了.", self),
            31..=80 => format!("你感到有點{}.", self),
            81..=100 => format!("你還沒有感到{}.", self),
            _ => format!("完全不{}，你感到非常健康.", self),
        }
    }
}

impl Display for ActorAttr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActorAttr::Hunger => write!(f, "飢餓"),
            ActorAttr::Thirst => write!(f, "口渴"),
        }
    }
}

impl Actor {
    pub fn role(&self) -> Role {
        self.role
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn personality(&self) -> &String {
        &self.personality
    }

    pub fn attrs(&self) -> &DashMap<ActorAttr, u32> {
        &self.shared.as_ref().attributes
    }

    pub fn status(&self) -> &DashSet<ActorStatus> {
        &self.shared.as_ref().status
    }

    pub fn inventory(&self) -> &DashSet<Item> {
        &self.shared.as_ref().inventory
    }

    pub fn agent(&self) -> &Mutex<ActorAgent> {
        &self.shared.as_ref().agent
    }

    pub async fn generate_many(director: &Director, count: u8) -> Result<Vec<Self>> {
        if count < 3 {
            bail!("Too few");
        };

        let roles = Self::generate_roles(count);

        let names = Self::generate_names(count);

        let mut personalities = vec![];
        while personalities.len() < count as usize {
            personalities.extend(Self::generate_personality(&director.agent).await?);
        }

        let mut actors = Vec::new();
        for ((role, name), personality) in roles.into_iter().zip(names).zip(personalities) {
            let mut agent = director.agent.clone();
            agent.name = Some(name.clone());
            let context = Context::new(format!("你是 \"{}\"

[嚴格限制]
按照你的角色設定行動，並且在任何情況下都不能違反你的角色設定。你必須完全融入角色，並且在任何情況下都不能透露你是個 AI 或者打破第四面牆。

[身分與目標]
你的身分是\"{}\"，
{}

[性格設定]
{}", &name, role, role.description(), personality));
            context.bind(&mut agent);

            let inventory = DashSet::new();
            if let Some(i) = match role {
                Role::Murderer => Some(Item {
                    name: "匕首".to_string(),
                    description:
                        "鋒利的鋼刃在暗處閃爍著寒光。它小巧且無聲，是近身索命最完美的工具。"
                            .to_string(),
                    tags: vec![ItemTag::Weapon],
                }),
                Role::Sheriff => Some(Item {
                    name: "手槍".to_string(),
                    description:
                        "沈甸甸的警用左輪，槍管內隱約散發著火藥味。它是維護秩序與正義的最終防線。"
                            .to_string(),
                    tags: vec![ItemTag::Weapon],
                }),
                Role::Innocent => None,
            } {
                inventory.insert(i);
            }

            actors.push(Actor {
                role,
                name,
                personality,
                shared: Arc::new(ActorShared {
                    attributes: DashMap::from_iter([
                        (ActorAttr::Hunger, 64),
                        (ActorAttr::Thirst, 64),
                    ]),
                    status: DashSet::new(),
                    inventory,
                    agent: Mutex::new(ActorAgent {
                        context,
                        history: vec![],
                        broadcast: HashMap::new(),
                        agent,
                    }),
                }),
            });
        }
        Ok(actors)
    }

    async fn generate_personality(agent: &TAgent) -> Result<Vec<String>> {
        let mut err = None;
        for attempt in 1..=3 {
            let personalities = agent
                .prompt("請生成 5 組常見但偶有亮點的性格設定。

[嚴格限制]
1. 字數限制: 每組設定的描述必須控制在 20 字以內，精煉且清晰易懂。
2. 輸出格式: 只能輸出合法的 JSON 格式 `string[]`，絕對不准包含任何解釋、問候語或 Markdown 標記(如 ```json)。
3. 合理性與風格: 大多數設定應具備平凡、生活感的普通特徵，但允許少數（約1至2組）帶有較為鮮明、有趣的個人怪癖或特色，以增添趣味。
4. 剝離身分標籤: **描述中絕對不可包含任何職業、社會身分或年齡角色名詞**（如：社區委員、上班族、老人、學生、專家等），必須純粹聚焦於「性格特質」與「行為/說話習慣」。

[輸出範例]
[\"熱心但說話容易失焦，常不自覺把話題扯遠\", \"性格溫和，習慣在每句話的句尾加上語氣詞\", \"做事按部就班，喜歡把手邊的物品按大小對齊\", \"隨性不拘小節，但對飲料的冰塊比例異常堅持\", \"沉穩少言，偶爾會用極具反差的冷幽默回應\"]")
                .await?;
            match serde_json::from_str::<Vec<String>>(&personalities) {
                Ok(list) => return Ok(list),
                Err(e) => {
                    error!("Attempt {}: Failed to parse personalities: {}", attempt, e);
                    err = Some(e);
                }
            }
        }
        Err(err.unwrap())?
    }

    fn generate_names(count: u8) -> Vec<String> {
        let family_names = [
            "陳", "林", "李", "王", "張", "劉", "黃", "楊", "吳", "趙", "周", "徐", "孫", "馬",
            "朱", "胡", "郭", "何", "高", "羅", "鄭", "梁", "謝", "韓", "唐", "馮", "宋", "程",
            "曹", "彭",
        ];

        let given_names = [
            "冠宇", "欣怡", "志豪", "雅婷", "俊宏", "佩君", "承恩", "詠晴", "宇軒", "婉婷", "嘉宏",
            "淑芬", "建廷", "美玲", "柏翰", "慧君", "子軒", "鈺婷", "冠廷", "雅玲", "智傑", "詩涵",
            "哲瑋", "曉薇", "郁翔", "曼婷", "育誠", "佳蓉", "宗翰", "宜蓁", "祥宇", "芷萱", "振宇",
            "惠雯", "家誠", "雅雯", "宏明", "雅芳", "志強", "郁婷",
        ];

        let mut names = vec![];
        let mut rng = rand::rng();
        while names.len() < count as usize {
            let f = family_names.choose(&mut rng).unwrap();
            let g = given_names.choose(&mut rng).unwrap();
            if !names.contains(&format!("{}{}", f, g)) {
                names.push(format!("{}{}", f, g));
            }
        }

        names
    }

    fn generate_roles(count: u8) -> Vec<Role> {
        let mut roles = vec![Role::Innocent; count as usize];
        roles[0] = Role::Murderer;
        roles[1] = Role::Sheriff;

        let mut rng = rand::rng();
        roles.shuffle(&mut rng);

        roles
    }
}

impl PartialEq for Actor {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }
}

impl Debug for Actor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Actor {{ name: {}, role: {}, personality: {} }}",
            self.name, self.role, self.personality
        )
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ActorStatus {
    Dead,
}

impl Display for ActorStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActorStatus::Dead => write!(f, "死亡"),
        }
    }
}

pub struct ActorAgent {
    pub context: Context,
    pub history: Vec<Message>,
    pub broadcast: HashMap<&'static str, broadcast::Receiver<Message>>,

    pub agent: TAgent,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActorAction {
    pub action: String,
    pub content: String,
    pub thought: String,
    #[serde(default)]
    pub talk: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActorTalk {
    pub thought: String,
    pub talk: String,
}

impl ActorAgent {
    /// sync broadcast and context
    pub fn sync(&mut self) -> Result<()> {
        for rx in self.broadcast.values_mut() {
            while let Ok(msg) = rx.try_recv() {
                self.history.push(msg);
            }
        }
        self.context.bind(&mut self.agent);
        Ok(())
    }
    pub async fn talk(&mut self, prompt: &str) -> Result<ActorTalk> {
        let prompt = Message::user(format!(
            "請你嚴格遵守以下輸出格式規則，**絕對不要**輸出任何額外的解釋、問候語或在 JSON 範圍外的思考過程。必須確保輸出為純粹且合法的 JSON 格式。

### [輸出規則]
嚴格按照以下 JSON 結構輸出，每次只能輸出單一個 JSON 物件。該物件必須包含 `thought` 和 `talk` 兩個鍵值。任何不符合此 JSON 格式的輸出都將被視為無效。

### [JSON 欄位定義]
* **`thought`**：字串。描述你的思考過程和理由。
* **`talk`**：字串。你想說的話；只能包含單句話，不得換行；若選擇沉默則填入空字串 `\"\"`。

{}",
            prompt
        ));

        for _ in 1..=3 {
            let response = match self.agent.chat(prompt.clone(), self.history.clone()).await {
                Ok(res) => res,
                Err(e) => {
                    error!("Failed to get response: {}", e);
                    continue;
                }
            };

            match serde_json::from_str(&response) {
                Ok(r) => {
                    self.history.push(Message::assistant(response.clone()));
                    return Ok(r);
                }
                Err(_) => warn!("Failed to parse response: {}", response),
            }
        }
        bail!("Failed to get valid talk after 3 attempts");
    }

    pub async fn action(&mut self, prompt: &str) -> Result<ActorAction> {
        let prompt = Message::user(format!(
            "請你嚴格遵守以下輸出格式規則，**絕對不要**輸出任何額外的解釋、問候語或在 JSON 範圍外的思考過程。必須確保輸出為純粹且合法的 JSON 格式。

### [輸出規則]
嚴格按照以下 JSON 結構輸出，每次只能輸出單一個 JSON 物件，代表一個動作。該物件必須包含 `thought`、`action`、`content` 和 `talk` 四個鍵值。任何不符合此 JSON 格式的輸出都將被視為無效。

### [JSON 欄位定義]
* **`thought`**：字串。描述你在做出該行為前的思考過程和理由，請確保這部分內容簡潔明了，直接反映你的內心想法和判斷。
* **`action`**：字串。行為名稱，必須完全匹配。
* **`content`**：字串。根據行為名稱的不同而有所區別，若無對應內容則填入空字串 `\"\"`。
* **`talk`**：字串。在執行行動的同時，你想對周圍的人說的話；只能包含單句話，不得換行；若選擇沉默則填入空字串 `\"\"`。

{}",
            prompt
        ));

        for _ in 1..=3 {
            let response = match self.agent.chat(prompt.clone(), self.history.clone()).await {
                Ok(res) => res,
                Err(e) => {
                    error!("Failed to get response: {}", e);
                    continue;
                }
            };

            match serde_json::from_str(&response) {
                Ok(r) => {
                    self.history.push(Message::assistant(response.clone()));
                    return Ok(r);
                }
                Err(_) => warn!("Failed to parse response: {}", response),
            }
        }
        bail!("Failed to get valid action after 3 attempts");
    }
}
