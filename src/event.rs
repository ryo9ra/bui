use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self as cevent, KeyEvent, KeyEventKind};

pub enum Event {
    Input(KeyEvent),
    Tick,
    TaskResult(TaskId, std::result::Result<Outcome, String>),
}

pub type TaskId = u64;

pub enum Outcome {
    Fetched,
    Pulled,
    Pushed,
    RemoteBranchDeleted { full_name: String },
}

pub struct EventChannel {
    rx: Receiver<Event>,
    pub tx: Sender<Event>,
}

impl EventChannel {
    pub fn recv(&self) -> Result<Event> {
        Ok(self.rx.recv()?)
    }
}

pub struct EventHandles {
    _input: JoinHandle<()>,
    _tick: JoinHandle<()>,
}

pub fn start_event_threads() -> (EventChannel, EventHandles) {
    let (tx, rx) = channel::<Event>();

    let tx_input = tx.clone();
    let input = thread::spawn(move || {
        loop {
            match cevent::poll(Duration::from_millis(100)) {
                Ok(true) => match cevent::read() {
                    Ok(cevent::Event::Key(k)) if k.kind == KeyEventKind::Press => {
                        if tx_input.send(Event::Input(k)).is_err() {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => return,
                },
                Ok(false) => {}
                Err(_) => return,
            }
        }
    });

    let tx_tick = tx.clone();
    let tick = thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(250));
            if tx_tick.send(Event::Tick).is_err() {
                return;
            }
        }
    });

    (
        EventChannel { rx, tx },
        EventHandles {
            _input: input,
            _tick: tick,
        },
    )
}
