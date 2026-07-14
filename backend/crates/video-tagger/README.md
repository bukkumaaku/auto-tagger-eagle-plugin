# video-tagger

Extracts temporary video frames with FFmpeg and tags them with an already loaded `wd-tagger` session.

The caller supplies the FFmpeg and FFprobe executable paths. This crate does not discover, install, or bundle either executable.

Processing lifecycle:

1. Read the video duration with FFprobe.
2. Create a unique directory below the operating system temp directory.
3. Extract `frame_count` PNG frames with FFmpeg.
4. Send frame paths to `wd-tagger` in `batch_size` chunks.
5. Return per-frame tags and merged video tags. Duplicate tags retain their highest frame score.
6. Delete the temporary frame directory on both success and failure.

Only one video-tagging call can be active in the worker process at a time.
