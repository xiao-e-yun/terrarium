use std::{cell::RefCell, fmt::Display};

#[derive(Debug, Default)]
pub struct Pending {
    pub count: RefCell<usize>,
}

impl Pending {
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
                return word.to_string();
            } else {
                return format!("{}{}", &word[..time], &prev[time..]);
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
        let text = Self::get_morphed_text(count, &Self::DEFAULT_WORDS);
        *self.count.borrow_mut() += 1;
        write!(f, "{} {}", indicator, text)
    }
}
