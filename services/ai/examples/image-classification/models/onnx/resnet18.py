from torchvision import models
import torch

resnet18 = models.resnet18(pretrained=True)

# Export the model to ONNX.
image_height = 224
image_width = 224
channels = 3
x = torch.randn(1, channels, image_height, image_width, requires_grad=True)
torch_out = resnet18(x)
torch.onnx.export(resnet18,                     # model being run
                  x,                            # model input (or a tuple for multiple inputs)
                  "resnet18.onnx",              # where to save the model (can be a file or file-like object)
                  export_params=True,           # store the trained parameter weights inside the model file
                  do_constant_folding=True,     # whether to execute constant folding for optimization
                  input_names = ['input'],      # the model's input names
                  output_names = ['output'])    # the model's output names