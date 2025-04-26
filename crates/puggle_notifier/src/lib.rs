use std::{ffi::OsStr, os::unix::fs::MetadataExt, path::Path};

use notify::{
    event::{CreateKind, DataChange, ModifyKind, RemoveKind},
    FsEventWatcher, RecursiveMode, Watcher,
};
use thiserror::Error;
use tokio::sync::mpsc::Receiver;

pub struct Handle {
    pub notifier: FsEventWatcher,
    pub rx: Receiver<notify::Result<notify::Event>>,
}

#[derive(Debug, Error)]
#[error(transparent)]
pub struct HandleError(#[from] notify::Error);

impl Handle {
    pub fn new() -> Result<Self, HandleError> {
        let (tx, rx) = tokio::sync::mpsc::channel::<notify::Result<notify::Event>>(1);

        let notifier = notify::RecommendedWatcher::new(
            move |res| {
                futures::executor::block_on(async {
                    tx.send(res).await.unwrap();
                })
            },
            notify::Config::default().with_compare_contents(true),
        )?;

        Ok(Self { notifier, rx })
    }

    pub async fn watch(&mut self, path: &Path) -> notify::Result<()> {
        self.notifier.watch(path, RecursiveMode::Recursive)?;

        while let Some(event) = self.rx.recv().await {
            let event = event?;
            if event.paths.iter().any(|a| {
                a.file_name() == Some(OsStr::new("puggle.yaml"))
                    || a.extension() == Some(OsStr::new("md"))
            }) {
                let path = event.paths.first().unwrap();
                let huh = std::fs::metadata(path);
                println!("{:#?}", huh.unwrap().ino());
                match event.kind {
                    notify::EventKind::Create(CreateKind::File) => {
                        println!("created a file {:#?}", event)
                    }
                    notify::EventKind::Modify(ModifyKind::Data(DataChange::Content)) => {
                    }
                    notify::EventKind::Remove(RemoveKind::File)
                    | notify::EventKind::Remove(RemoveKind::Folder) => {
                        println!("removed a file/dir {:#?}", event)
                    }
                    _ => (),
                }
            }
        }

        Ok(())
    }
}
