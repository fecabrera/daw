use std::sync::mpsc::{Receiver, TryRecvError};

/// A completed one-shot worker, including a disconnect before it could send.
pub(super) fn poll<T, E>(
    receiver: &Receiver<Result<T, E>>,
    disconnected: impl FnOnce() -> E,
) -> Option<Result<T, E>> {
    match receiver.try_recv() {
        Ok(result) => Some(result),
        Err(TryRecvError::Empty) => None,
        Err(TryRecvError::Disconnected) => Some(Err(disconnected())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polls_pending_results_and_disconnects_lazily() {
        let (sender, receiver) = std::sync::mpsc::channel();
        assert_eq!(poll(&receiver, || panic!("worker is connected")), None);
        sender.send(Ok(3)).unwrap();
        assert_eq!(
            poll(&receiver, || panic!("worker sent a result")),
            Some(Ok(3))
        );
        sender.send(Err("work failed")).unwrap();
        drop(sender);
        assert_eq!(poll(&receiver, || "disconnected"), Some(Err("work failed")));
        assert_eq!(
            poll(&receiver, || "disconnected"),
            Some(Err("disconnected"))
        );
    }
}
