# ONNX Model Assets

Place ONNX model folders here for Tier 3 inference.

Expected default layout:

```
models/
  qwen2.5-coder-1.5b/
    model.onnx
    tokenizer.json
```

The app discovers models from this directory at startup and can load them with the `load_model` IPC command.
