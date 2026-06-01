use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use tokio::sync::mpsc;

pub enum AppEvent {
    Key(crossterm::event::KeyEvent),
    Tick,
}

pub struct EventReader {
    rx: mpsc::UnboundedReceiver<AppEvent>,
}

impl EventReader {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            loop {
                if event::poll(tick_rate).is_ok_and(|ready| ready) {
                    if let Ok(Event::Key(key)) = event::read()
                        && key.kind == KeyEventKind::Press
                        && tx.send(AppEvent::Key(key)).is_err()
                    {
                        return;
                    }
                } else if tx.send(AppEvent::Tick).is_err() {
                    return;
                }
            }
        });

        Self { rx }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}
