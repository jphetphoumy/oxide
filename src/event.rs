use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind, MouseEvent};
use tokio::sync::mpsc;

pub enum AppEvent {
    Key(crossterm::event::KeyEvent),
    Mouse(MouseEvent),
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
                    match event::read() {
                        Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                            if tx.send(AppEvent::Key(key)).is_err() {
                                return;
                            }
                        }
                        Ok(Event::Mouse(mouse)) if tx.send(AppEvent::Mouse(mouse)).is_err() => {
                            return;
                        }
                        _ => {}
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
