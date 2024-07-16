extern crate slint;
extern crate rodio;
extern crate tokio;
extern crate rfd;
mod loadfile;
#[allow(clippy::all)]
pub mod generated_code {
    slint::include_modules!();
}
pub use generated_code::*;
use loadfile::{load_files, Song};
use slint::{ModelRc, SharedString, VecModel};

use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::fs;
use std::path::Path;
use rodio::{OutputStream, OutputStreamHandle, Sink, Decoder};
use std::io::BufReader;
use tokio::task;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender, Receiver};



struct MusicPlayer {
    songs: Vec<Song>,
    current_index: usize,
    sink: Arc<Mutex<Sink>>,
    stream_handle: OutputStreamHandle,
    loop_enabled: Arc<Mutex<bool>>,
    tx: Sender<PlayerCommand>,
}

impl MusicPlayer {
    fn new(songs: Vec<Song>, stream_handle: OutputStreamHandle, tx: Sender<PlayerCommand>) -> Self {
        let sink = Sink::try_new(&stream_handle).unwrap();
        Self {
            songs,
            current_index: 0,
            sink: Arc::new(Mutex::new(sink)),
            stream_handle,
            loop_enabled: Arc::new(Mutex::new(false)),
            tx,
        }
    }

    fn play(&self) {
        let sink = Arc::clone(&self.sink);
        let stream_handle = self.stream_handle.clone();
        let song = self.songs[self.current_index].clone();
        let loop_enabled = Arc::clone(&self.loop_enabled);
        let tx = self.tx.clone();
        task::spawn_blocking(move || {
            let mut sink = sink.lock().unwrap();
            sink.stop();
            let file = std::fs::File::open(song.path).unwrap();
            let source = Decoder::new(BufReader::new(file)).unwrap();
            sink.append(source);
            sink.play();
            // let loop_enabled = Arc::clone(&loop_enabled);
            // let tx = tx.clone();
            // sink.set_finish_callback(move || {
            //     if *loop_enabled.lock().unwrap() {
            //         let mut sink = sink.clone();
            //         let file = std::fs::File::open(song.clone()).unwrap();
            //         let source = Decoder::new(BufReader::new(file)).unwrap();
            //         sink.append(source);
            //         sink.play();
            //     } else {
            //         let _ = tx.try_send(PlayerCommand::Next);
            //     }
            // });
        });
    }


    fn pause(&self) {
        let sink = self.sink.lock().unwrap();
        sink.pause();
    }

    fn stop(&self) {
        let sink = self.sink.lock().unwrap();
        sink.stop();
    }

    fn next(&mut self) {
        self.current_index = (self.current_index + 1) % self.songs.len();
        self.play();
    }

    fn previous(&mut self) {
        if self.current_index == 0 {
            self.current_index = self.songs.len() - 1;
        } else {
            self.current_index -= 1;
        }
        self.play();
    }

    fn toggle_loop(&self) {
        let mut loop_enabled = self.loop_enabled.lock().unwrap();
        *loop_enabled = !*loop_enabled;
    }
}

enum PlayerCommand {
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    ToggleLoop,
    LoadSongs(Vec<Song>),
}

#[tokio::main]
async fn main() {
    let app = App::new().unwrap();
    let app_weak = app.as_weak();

    let (stream, stream_handle) = OutputStream::try_default().unwrap();
    let (tx, mut rx): (Sender<PlayerCommand>, Receiver<PlayerCommand>) = mpsc::channel(1);
    let player = Arc::new(Mutex::new(MusicPlayer::new(Vec::new(), stream_handle, tx.clone())));

    app.on_play({
        let tx = tx.clone();
        move || {
            let _ = tx.try_send(PlayerCommand::Play);
        }
    });

    app.on_pause({
        let tx = tx.clone();
        move || {
            let _ = tx.try_send(PlayerCommand::Pause);
        }
    });

    app.on_stop({
        let tx = tx.clone();
        move || {
            let _ = tx.try_send(PlayerCommand::Stop);
        }
    });

    app.on_next({
        let tx = tx.clone();
        move || {
            let _ = tx.try_send(PlayerCommand::Next);
        }
    });

    app.on_previous({
        let tx = tx.clone();
        move || {
            let _ = tx.try_send(PlayerCommand::Previous);
        }
    });

    // app.on_toggle_loop({
    //     let tx = tx.clone();
    //     move || {
    //         let _ = tx.try_send(PlayerCommand::ToggleLoop);
    //     }
    // });

    app.on_open_folder({
        let app_weak = app_weak.clone();
        let tx = tx.clone();
        move || {
            let app_weak = app_weak.clone();
            let tx = tx.clone();
            tokio::spawn(async move {
                let dialog = rfd::AsyncFileDialog::new().pick_folder().await;
                if let Some(folder) = dialog {
                    let folder_path = folder.path().display().to_string();
                    let songs = load_files(&folder_path);
                    let song = songs.iter().map(|song|{
                        song.clone().title.into()
                    }).collect::<Vec<SharedString>>();
                    let _ = app_weak.upgrade_in_event_loop(move |ui|{
                        ui.set_songs(ModelRc::from(Rc::new(VecModel::from(song))))
                    });
                    let _ = tx.send(PlayerCommand::LoadSongs(songs)).await;
                }
            });
        }
    });

    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(command) = rx.recv() => {
                    let mut player = player.lock().unwrap();
                    match command {
                        PlayerCommand::Play => player.play(),
                        PlayerCommand::Pause => player.pause(),
                        PlayerCommand::Stop => player.stop(),
                        PlayerCommand::Next => player.next(),
                        PlayerCommand::Previous => player.previous(),
                        PlayerCommand::ToggleLoop => player.toggle_loop(),
                        PlayerCommand::LoadSongs(songs) => player.songs = songs,
                    }
                }
            }
        }
    });

    app.run().unwrap();
}


