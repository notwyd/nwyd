use std::sync::mpsc::{Receiver, SyncSender};

pub(crate) fn spawn<T: Send + 'static, R: Send + 'static>(
    name: &str,
    requests: Receiver<T>,
    replies: SyncSender<R>,
    mut prepare: impl FnMut(T) -> R + Send + 'static,
) -> std::io::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            while let Ok(request) = requests.recv() {
                if replies.send(prepare(request)).is_err() {
                    break;
                }
            }
        })
        .map(|_| ())?;

    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;

        wasm_bindgen_futures::spawn_local(async move {
            loop {
                crate::web::delay(16).await;

                match requests.try_recv() {
                    Ok(request) => {
                        let mut reply = prepare(request);

                        loop {
                            match replies.try_send(reply) {
                                Ok(()) => break,
                                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => return,
                                Err(std::sync::mpsc::TrySendError::Full(value)) => {
                                    reply = value;

                                    crate::web::delay(16).await;
                                }
                            }
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }
            }
        });
    }

    Ok(())
}
