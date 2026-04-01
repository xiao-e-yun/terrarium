use std::{cell::RefCell, fmt::Display, io::{self, Write}};


#[derive(Debug)]
pub struct Pending {
    pub count: RefCell<usize>,
    pub words: &'static [&'static str],
}

impl Pending {
    pub fn new(words: &'static [&'static str]) -> Self {
        Self {
            count: RefCell::new(0),
            words,
        }
    }

    /// A simple async function that keeps the pending indicator active.
    pub async fn active(&self) -> ! {
         loop {
            print!("\r\x1B[2K{}", self);
            io::stdout().flush().unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
         }
    }

    pub fn indicator(count: usize) -> &'static str {
        // · → + → *
        match count / 2 % 3 {
            0 => "·",
            1 => "+",
            _ => "*",
        }
    }

    const DEFAULT_WORDS: [&'static str; 4] = ["Pondering", "Reflecting", "Reasoning", "Concluding"];
    pub fn get_morphed_text(time: usize, words: &[&'static str]) -> String {
        let pause_steps = 20;

        let words = words.iter().map(|w|w.chars().collect()).collect::<Vec<Vec<_>>>();
        let words_len = words.iter().map(|w| w.len()).collect::<Vec<_>>();
        let total = words_len.iter().sum::<usize>() + words.len() * pause_steps;

        let mut time = (time + words.last().unwrap().len()) % total;
        let mut prev = words.last().unwrap();
        for (word, l) in words.iter().zip(words_len) {
            if time >= l + pause_steps {
                time -= l + pause_steps;
                prev = word;
                continue;
            } else if time >= l {
                return word.iter().collect::<String>();
            } else {
                // index by chars
                let word = word[..time].iter().collect::<String>();
                let prev_index = time.min(prev.len());
                let prev = prev[prev_index..].iter().collect::<String>();
                return format!("{}{}", &word, &prev);
            }
        };
        unreachable!()
    }
}

impl Display for Pending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Thinking ->
        let count = *self.count.borrow();
        let indicator = Self::indicator(count);
        let text = Self::get_morphed_text(count, self.words);
        *self.count.borrow_mut() += 1;
        write!(f, "{} {}", indicator, text)
    }
}

impl Default for Pending {
    fn default() -> Self {
        Self {
            count: RefCell::new(0),
            words: &Self::DEFAULT_WORDS,
        }
    }
}

#[macro_export]
macro_rules! pending_until {
    ($handle:expr, $words:expr) => {
        {
            let pending = Pending::new(&$words);
            select! {
                output = $handle => {
                    print!("\n");
                    output
                },
                _ = pending.active() => unreachable!()
            }
        }
    };
}

