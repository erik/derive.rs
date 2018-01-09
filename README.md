# derive.rs

![demo](https://i.imgur.com/SXYasIX.gif)

Rust reimplementation of [derive](https://github.com/erik/derive).

```
cargo run --release --                                           \
          --bounds '34.205911 -119.009399 33.709276 -118.026123' \
          --width 2000                                           \
          --output heatmap.png                                   \
          --ppm-stream                                           \
          --frame-rate 1950                                      \
          ~/Downloads/strava-data-dump/                          \
| ffmpeg -i - -y heatmap.mp4
```
