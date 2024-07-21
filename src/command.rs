use crate::App;
use rodio::{Decoder, OutputStreamHandle, Sink, Source};
use slint::Weak;
use std::{
    io::BufReader,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{sync::mpsc::Sender, task, time};

use crate::{loadfile::Song, PlayerCommand};

enum PlayerState {
    Playing,
    Paused,
    Stopped,
}

pub struct MusicPlayer {
    pub songs: Vec<Song>,
    current_index: usize,
    sink: Arc<Mutex<Sink>>,
    stream_handle: OutputStreamHandle,
    loop_enabled: Arc<Mutex<bool>>,
    state: Arc<Mutex<PlayerState>>,
    tx: Sender<PlayerCommand>,
    app: Weak<App>,
}

impl MusicPlayer {
    pub fn new(
        stream_handle: OutputStreamHandle,
        tx: Sender<PlayerCommand>,
        app: Weak<App>,
    ) -> Self {
        let sink = Sink::try_new(&stream_handle).unwrap();
        Self {
            songs: Vec::default(),
            current_index: 0,
            sink: Arc::new(Mutex::new(sink)),
            stream_handle,
            loop_enabled: Arc::new(Mutex::new(false)),
            state: Arc::new(Mutex::new(PlayerState::Stopped)),
            tx,
            app,
        }
    }

    pub fn play(&self) {
        let current_state = {
            let state = self.state.lock().unwrap();
            state
        };

        match *current_state {
            PlayerState::Paused => {
                let sink = self.sink.lock().unwrap();
                sink.play();
                let mut state = self.state.lock().unwrap();
                *state = PlayerState::Playing;
            }
            _ => {
                let song = {
                    let sink = self.sink.lock().unwrap();
                    sink.stop();
                    self.songs[self.current_index].clone()
                };
                let sink = Arc::clone(&self.sink);
                // let _stream_handle = self.stream_handle.clone();
                let loop_enabled = Arc::clone(&self.loop_enabled);
                let tx = self.tx.clone();
                let state = Arc::clone(&self.state);
                let _ = self.app.upgrade_in_event_loop(move |ui| {
                    ui.set_maxtime(song.duration.clone() as i32);
                });

                task::spawn_blocking(move || {
                    let file = std::fs::File::open(song.path.clone()).unwrap();
                    let source = Decoder::new(BufReader::new(file)).unwrap();

                    {
                        let sink = sink.lock().unwrap();
                        sink.append(source);
                        sink.play();
                    }

                    {
                        let mut state = state.lock().unwrap();
                        *state = PlayerState::Playing;
                    }

                    task::spawn(async move {
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            let is_empty = {
                                let sink = sink.lock().unwrap();
                                sink.empty()
                            };

                            if is_empty {
                                if *loop_enabled.lock().unwrap() {
                                    let file = std::fs::File::open(song.path.clone()).unwrap();
                                    let source = Decoder::new(BufReader::new(file)).unwrap();
                                    {
                                        let sink = sink.lock().unwrap();
                                        sink.append(source);
                                        sink.play();
                                    }
                                } else {
                                    let _ = tx.try_send(PlayerCommand::Next);
                                }
                                break;
                            }
                        }
                    });
                });
            }
        }
    }

    pub fn pause(&self) {
        let sink = self.sink.lock().unwrap();
        sink.pause();
        let mut state = self.state.lock().unwrap();
        *state = PlayerState::Paused;
    }

    pub fn stop(&self) {
        let sink = self.sink.lock().unwrap();
        sink.stop();
        let mut state = self.state.lock().unwrap();
        *state = PlayerState::Stopped;
    }

    pub fn next(&mut self) {
        self.current_index = (self.current_index + 1) % self.songs.len();
        self.play();
    }

    pub fn previous(&mut self) {
        if self.current_index == 0 {
            self.current_index = self.songs.len() - 1;
        } else {
            self.current_index -= 1;
        }
        self.play();
    }

    pub fn select(&mut self, u: i32) {
        // let sink = self.sink.lock().unwrap();
        // sink.stop();
        // let mut state = self.state.lock().unwrap();
        // *state = PlayerState::Stopped;
        self.current_index = u as usize;
        // self.state = PlayState::Playing;
        self.play();
    }

    pub fn toggle_loop(&self, b: bool) {
        let mut loop_enabled = self.loop_enabled.lock().unwrap();
        *loop_enabled = b;
    }
}
