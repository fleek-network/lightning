# This script creates an onnx model with builtin pre and post processing.
# This can be useful in environments where a lighter client is preferred.
import onnx
from onnxruntime_extensions.tools.pre_post_processing import *

new_input = create_named_value('image', onnx.TensorProto.UINT8, ['num_bytes'])
pipeline = PrePostProcessor([new_input], onnx_opset=18)

pipeline.add_pre_processing(
    [
        ConvertImageToBGR(),
        Resize(224),
        CenterCrop(224, 224),
        ChannelsLastToChannelsFirst(),
        ImageBytesToFloat(),
        Unsqueeze(axes=[0]),
    ]
)

pipeline.add_post_processing([Squeeze(0)])

# See Rust vision examples for a script that can generate this onnx model file.
model = onnx.load('resnet34.onnx')

new_model = pipeline.run(model)

onnx.save_model(new_model, 'resnet34.with_pre_post_processing.onnx')