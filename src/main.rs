extern crate rfd;
extern crate rodio;
extern crate slint;
extern crate tokio;
mod loadfile;
#[allow(clippy::all)]
pub mod generated_code {
    slint::include_modules!();
}
use command::MusicPlayer;
pub use generated_code::*;
use loadfile::{run_load, Song};
use rodio::OutputStream;
use slint::{ModelRc, SharedString, VecModel};

use std::rc::Rc;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
pub mod command;

pub enum PlayerCommand {
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    ToggleLoop { bool: bool },
    LoadSongs(Vec<Song>),
    Select { u: i32 },
}

#[tokio::main]
async fn main() {
    let app = App::new().unwrap();
    let app_weak = app.as_weak();

    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let (tx, mut rx): (Sender<PlayerCommand>, Receiver<PlayerCommand>) = mpsc::channel(1);
    let player = Arc::new(Mutex::new(MusicPlayer::new(
        stream_handle,
        tx.clone(),
        app_weak.clone(),
    )));

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

    app.on_toggle_loop({
        let tx = tx.clone();
        move |bool| {
            let _ = tx.try_send(PlayerCommand::ToggleLoop { bool });
        }
    });

    app.on_open_folder({
        let app_weak = app_weak.clone();
        let tx = tx.clone();
        move || {
            let app_weak = app_weak.clone();
            let tx = tx.clone();
            tokio::spawn(async move {
                let songs = run_load(app_weak.clone()).await.unwrap();
                // let song = songs.iter().map(|song|{
                //     song.clone().title.into()
                // }).collect::<Vec<SharedString>>();
                // let _ = app_weak.upgrade_in_event_loop(move |ui|{
                //     ui.set_songs(ModelRc::from(Rc::new(VecModel::from(song))))
                // });
                let _ = tx.send(PlayerCommand::LoadSongs(songs)).await;
            });
        }
    });

    app.on_select({
        let tx = tx.clone();
        move |u| {
            let _ = tx.try_send(PlayerCommand::Select { u });
        }
    });

    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(command) = rx.recv() => {
                    let mut player = player.lock().unwrap();
                    match command {
                        PlayerCommand::Play=>player.play(),
                        PlayerCommand::Pause=>player.pause(),
                        PlayerCommand::Stop=>player.stop(),
                        PlayerCommand::Next=>player.next(),
                        PlayerCommand::Previous=>player.previous(),
                        PlayerCommand::ToggleLoop{bool}=>player.toggle_loop(bool),
                        PlayerCommand::LoadSongs(songs)=>player.songs=songs,
                        PlayerCommand::Select { u } => player.select(u), }
                }
            }
        }
    });

    app.run().unwrap();
}
