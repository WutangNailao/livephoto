# Minimal Web Player

## Run

Use any static file server from the repository root or from the `web/` directory.

Example:

```bash
cd /Users/nailao/Code/livephoto/web
python3 -m http.server 8080
```

Then open:

[http://localhost:8080](http://localhost:8080)

## What It Does

- reads a local `.livephoto` file with the browser File API
- parses file header, TOC, `META`, `PHOT`, `VIDE`
- renders the cover image
- plays the video on click or press
- resets to the cover image after playback ends

## Limits

- no streaming parser yet
- no bridge chunk visualization
- no frame-accurate seek to `cover_timestamp_ms`
