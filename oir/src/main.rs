#![feature(drain_filter)]

#[macro_use]
extern crate lazy_static;

use crate::actor::ErrorBox;
use crate::actor::SystemMessage;

mod actor;
mod mailbox;
mod message_handler;
mod request_handler;

use crate::message_handler::message_actor;
use crate::message_handler::PingActor;
use crate::message_handler::PingMessage;

use crate::request_handler::request_actor;
use crate::request_handler::Stactor;
use crate::request_handler::StoreRequest;

use crate::mailbox::Mailbox;
use crate::mailbox::NamedMailbox;
use crate::mailbox::UnnamedMailbox;

use tokio;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;
    let _: Result<(), ErrorBox> = rt.block_on(async {
        let (ts0, rs0) = mpsc::channel::<SystemMessage>(512);
        let (tp0, rp0) = mpsc::channel::<PingMessage>(512);
        let h0 = message_actor(PingActor { subject: None }, rs0, rp0);

        let (ts1, rs1) = mpsc::channel::<SystemMessage>(512);
        let (tp1, rp1) = mpsc::channel::<PingMessage>(512);
        let h1 = message_actor(PingActor { subject: None }, rs1, rp1);

        // Pings
        tp0.send(PingMessage::Setup(tp1.clone())).await?;
        tp1.send(PingMessage::Setup(tp0.clone())).await?;
        tp0.send(PingMessage::Ping).await?;
        tokio::time::sleep(tokio::time::Duration::new(0, 1)).await;

        // Shutdown
        ts0.send(SystemMessage::Shutdown).await?;
        ts1.send(SystemMessage::Shutdown).await?;
        h0.await?;
        h1.await?;
        Ok(())
    });
    rt.block_on(async {
        let (mut unnamed_mailbox, _h0) = request_actor(Stactor::new());

        let mut vec = Vec::new();

        for i in 0..1024i32 {
            let mut unnamed_mailbox0 = unnamed_mailbox.clone();
            let (one_s, one_r) = oneshot::channel::<Option<i32>>();
            vec.push((
                one_r,
                tokio::spawn(async move {
                    let _ = unnamed_mailbox0
                        .send((one_s, StoreRequest::Set(i, i)))
                        .await;
                }),
            ));
        }

        for (r, h) in vec.drain(..) {
            h.await?;
            let _result = r.await?;
        }

        for i in 0..1024 {
            let (one_s, one_r) = oneshot::channel::<Option<i32>>();
            unnamed_mailbox.send((one_s, StoreRequest::Get(i))).await.unwrap();
            let result = one_r.await?;
            assert_eq!(Some(i), result);
        }
        Ok(())
    })
}
