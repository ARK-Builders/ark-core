use anyhow::Result;
use ratatui::{Terminal, prelude::Backend};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub trait Page {
    fn render(&self);
}

pub struct App<B: Backend> {
    id: String,
    term: Arc<RwLock<Terminal<B>>>,
}

impl<B: Backend> App<B> {
    pub fn new(term: Arc<RwLock<Terminal<B>>>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            term,
        }
    }

    pub fn run(&self) -> Result<()> {
        loop {}
        todo!();
    }
}
