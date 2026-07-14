# wd-tagger

Rust inference core for WD-family image taggers.

Supported model formats:

- `wd`: `selected_tags.csv`, NHWC BGR input, probability output.
- `cl`: `tag_mapping.json`, NCHW normalized BGR input, logit output.
- `camie`: `camie-tagger-v2-metadata.json`, NCHW ImageNet-normalized BGR input, logit output.

The crate owns image decoding, alpha flattening, square letterboxing, ONNX Runtime inference, threshold filtering, and score sorting. Worker session lifecycle and JSONL transport remain in `ai-worker`.

On Windows, `auto` tries DirectML and allows ONNX Runtime to fall back to CPU. `direct_ml` fails if DirectML cannot be registered. `cpu` does not register a GPU execution provider.
