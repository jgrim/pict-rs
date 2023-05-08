use crate::{
    error::{Error, UploadError},
    upload_manager::Details,
};
use actix_web::web;
use dashmap::{mapref::entry::Entry, DashMap};
use once_cell::sync::Lazy;
use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::oneshot::{Receiver, Sender};
use tracing::Span;

type OutcomeSender = Sender<(Details, web::Bytes)>;

type ProcessMap = DashMap<PathBuf, Vec<OutcomeSender>>;

static PROCESS_MAP: Lazy<ProcessMap> = Lazy::new(DashMap::new);

struct CancelToken {
    span: Span,
    path: PathBuf,
    receiver: Option<Receiver<(Details, web::Bytes)>>,
}

pin_project_lite::pin_project! {
    pub(super) struct CancelSafeProcessor<F> {
        cancel_token: CancelToken,

        #[pin]
        fut: F,
    }
}

impl<F> CancelSafeProcessor<F>
where
    F: Future<Output = Result<(Details, web::Bytes), Error>>,
{
    pub(super) fn new(path: PathBuf, fut: F) -> Self {
        let entry = PROCESS_MAP.entry(path.clone());

        let (receiver, span) = match entry {
            Entry::Vacant(vacant) => {
                vacant.insert(Vec::new());
                let span = tracing::info_span!(
                    "Processing image",
                    path = &tracing::field::debug(&path),
                    completed = &tracing::field::Empty,
                );
                (None, span)
            }
            Entry::Occupied(mut occupied) => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                occupied.get_mut().push(tx);
                let span = tracing::info_span!(
                    "Waiting for processed image",
                    path = &tracing::field::debug(&path),
                );
                (Some(rx), span)
            }
        };

        CancelSafeProcessor {
            cancel_token: CancelToken {
                span,
                path,
                receiver,
            },
            fut,
        }
    }
}

impl<F> Future for CancelSafeProcessor<F>
where
    F: Future<Output = Result<(Details, web::Bytes), Error>>,
{
    type Output = Result<(Details, web::Bytes), Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().project();

        let span = &this.cancel_token.span;
        let receiver = &mut this.cancel_token.receiver;
        let path = &this.cancel_token.path;
        let fut = this.fut;

        span.in_scope(|| {
            if let Some(ref mut rx) = receiver {
                Pin::new(rx)
                    .poll(cx)
                    .map(|res| res.map_err(|_| UploadError::Canceled.into()))
            } else {
                fut.poll(cx).map(|res| {
                    let opt = PROCESS_MAP.remove(path);
                    res.map(|tup| {
                        if let Some((_, vec)) = opt {
                            for sender in vec {
                                let _ = sender.send(tup.clone());
                            }
                        }
                        tup
                    })
                })
            }
        })
    }
}

impl Drop for CancelToken {
    fn drop(&mut self) {
        if self.receiver.is_none() {
            let completed = PROCESS_MAP.remove(&self.path).is_none();
            self.span.record("completed", completed);
        }
    }
}
