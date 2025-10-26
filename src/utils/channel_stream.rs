// use crossbeam::channel;
// use iced::futures::{Stream, StreamExt};
// use std::{
//     pin::Pin,
//     task::{Context, Poll},
// };

// pub struct ChannelStream<T> {
//     rx: channel::Receiver<T>,
// }

// impl<T> From<channel::Receiver<T>> for ChannelStream<T> {
//     fn from(value: channel::Receiver<T>) -> Self { return Self { rx: value } }
// }

// impl<T> Stream for ChannelStream<T> {
//     type Item = T;

//     fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
//         let this = self.get_mut();

//         return match this.rx.recv() {
//             Ok(msg) => Poll::Ready(Some(msg)),
//             Err(_) => Poll::Ready(None),
//         };
//     }
// }

// pub struct ChannelTranformingStream<T, U> {
//     inner:       ChannelStream<T>,
//     transformer: fn(T) -> U,
// }

// impl<T, U> ChannelTranformingStream<T, U> {
//     pub fn new(rx: channel::Receiver<T>, transformer: fn(T) -> U) -> Self {
//         return Self {
//             inner: ChannelStream { rx },
//             transformer,
//         };
//     }
// }

// impl<T, U> Stream for ChannelTranformingStream<T, U> {
//     type Item = U;

//     fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
//         let this = self.get_mut();

//         return this.inner.poll_next_unpin(cx).map(|x| match x {
//             Some(item) => Some((this.transformer)(item)),
//             None => None,
//         });
//     }
// }
