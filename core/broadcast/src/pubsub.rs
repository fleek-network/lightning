use std::marker::PhantomData;

use async_trait::async_trait;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::PubSub;

pub struct PubSubI<T: LightningMessage + Clone> {
    message: PhantomData<T>,
}

impl<T: LightningMessage + Clone> Clone for PubSubI<T> {
    fn clone(&self) -> Self {
        Self {
            message: PhantomData,
        }
    }
}

#[async_trait]
impl<T: LightningMessage + Clone> PubSub<T> for PubSubI<T> {
    async fn send(&self, _msg: &T) {
        todo!()
    }

    async fn recv(&mut self) -> Option<T> {
        todo!()
    }
}
