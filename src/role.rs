use std::fmt::{Display};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Murderer,
    Sheriff,
    Innocent,
}

impl Role {
    pub fn description(&self) -> &'static str {
        match self {
            Role::Murderer => "核心目標: 時間內殺光所有平民，同時確保不暴露身分。
遊玩重點: 尋找作案時機。",
            Role::Sheriff => "核心目標： 帶領平民活下去，透過搜查與盤問找出真兇，並行使處決權制裁兇手。
遊玩重點： 權力與猜忌的平衡。必須在真假線索與平民的恐慌中做出正確判斷；同時要時刻提防被暗殺。",
            Role::Innocent => "遊玩重點： 稍有不慎就會成為兇手的目標，或是成為替罪羊。",
        }
    }
}

impl Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Murderer => write!(f,"殺手"),
            Role::Sheriff => write!(f,"警長"),
            Role::Innocent => write!(f,"平民"),
        }
    }
}
