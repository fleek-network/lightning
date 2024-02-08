#![allow(dead_code)]
use std::io;
use std::io::Cursor;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tch::nn::{Adam, ModuleT, OptimizerConfig, VarStore};
use tch::vision::dataset;
use tch::{CModule, Device, TrainableCModule};

use crate::libtorch::mnist;

pub struct Config {
    pub model: Bytes,
}
pub struct Dataset {
    pub train_images: Bytes,
    pub train_labels: Bytes,
    pub validation_images: Bytes,
    pub validation_labels: Bytes,
    pub labels: i64,
}

#[derive(Deserialize, Serialize)]
pub struct TrainingResult {
    pub test_accuracy: f64,
}

pub fn train(opts: Config, dataset: Dataset) -> anyhow::Result<TrainingResult> {
    let dataset = mnist::load_from_mem(dataset)?;
    let dataset = dataset::Dataset {
        train_images: dataset.train_images.view([-1, 1, 28, 28]),
        test_images: dataset.test_images.view([-1, 1, 28, 28]),
        ..dataset
    };
    let device = Device::Cpu;
    train_and_save_model(Cursor::new(opts.model), &dataset, device)?;
    let test_accuracy = load_trained_and_test_acc(&dataset, device)?;
    Ok(TrainingResult { test_accuracy })
}

fn train_and_save_model<M: io::Read>(
    mut model: M,
    dataset: &dataset::Dataset,
    device: Device,
) -> anyhow::Result<()> {
    let vs = VarStore::new(device);
    let mut trainable = TrainableCModule::load_data(&mut model, vs.root())?;
    trainable.set_train();
    let initial_acc = trainable.batch_accuracy_for_logits(
        &dataset.test_images,
        &dataset.test_labels,
        vs.device(),
        1024,
    );
    tracing::trace!("Initial accuracy: {:5.2}%", 100. * initial_acc);

    let mut opt = Adam::default().build(&vs, 1e-4)?;
    for epoch in 1..3 {
        for (images, labels) in dataset
            .train_iter(128)
            .shuffle()
            .to_device(vs.device())
            .take(50)
        {
            let loss = trainable
                .forward_t(&images, true)
                .cross_entropy_for_logits(&labels);
            opt.backward_step(&loss);
        }
        let test_accuracy = trainable.batch_accuracy_for_logits(
            &dataset.test_images,
            &dataset.test_labels,
            vs.device(),
            1024,
        );
        tracing::trace!("epoch: {:4} test acc: {:5.2}%", epoch, 100. * test_accuracy,);
    }

    // Todo: we have to fix this.
    // It might make sense to save this on the blockstore,
    // specially if there is no API to return a serialized model
    // that a validation step can consume.
    trainable.save("trained_model.pt")?;

    Ok(())
}

fn load_trained_and_test_acc(dataset: &dataset::Dataset, device: Device) -> anyhow::Result<f64> {
    let mut module = CModule::load_on_device("trained_model.pt", device)?;
    module.set_eval();
    let accuracy =
        module.batch_accuracy_for_logits(&dataset.test_images, &dataset.test_labels, device, 1024);
    tracing::trace!("Updated accuracy: {:5.2}%", 100. * accuracy);
    Ok(accuracy)
}
