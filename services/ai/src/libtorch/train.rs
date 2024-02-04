use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tch::nn::{Adam, ModuleT, OptimizerConfig, VarStore};
use tch::vision::dataset;
use tch::{CModule, Device, TrainableCModule};

pub struct Config {
    pub model: Bytes,
}
pub struct Dataset {}

#[derive(Deserialize, Serialize)]
pub struct TrainingResult {
    pub test_accuracy: f64,
}

pub fn train(_opts: Config, _dataset: Dataset) -> anyhow::Result<TrainingResult> {
    let dataset = tch::vision::mnist::load_dir("data")?;
    let dataset = dataset::Dataset {
        train_images: dataset.train_images.view([-1, 1, 28, 28]),
        test_images: dataset.test_images.view([-1, 1, 28, 28]),
        ..dataset
    };
    let device = Device::Cpu;
    train_and_save_model(&dataset, device)?;
    let test_accuracy = load_trained_and_test_acc(&dataset, device)?;
    Ok(TrainingResult { test_accuracy })
}

fn train_and_save_model(dataset: &dataset::Dataset, device: Device) -> anyhow::Result<()> {
    let vs = VarStore::new(device);
    let mut trainable = TrainableCModule::load("model.pt", vs.root())?;
    trainable.set_train();
    let initial_acc = trainable.batch_accuracy_for_logits(
        &dataset.test_images,
        &dataset.test_labels,
        vs.device(),
        1024,
    );
    println!("Initial accuracy: {:5.2}%", 100. * initial_acc);

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
        println!("epoch: {:4} test acc: {:5.2}%", epoch, 100. * test_accuracy,);
    }

    trainable.save("trained_model.pt")?;

    Ok(())
}

fn load_trained_and_test_acc(dataset: &dataset::Dataset, device: Device) -> anyhow::Result<f64> {
    let mut module = CModule::load_on_device("trained_model.pt", device)?;
    module.set_eval();
    let accuracy =
        module.batch_accuracy_for_logits(&dataset.test_images, &dataset.test_labels, device, 1024);
    println!("Updated accuracy: {:5.2}%", 100. * accuracy);
    Ok(accuracy)
}
