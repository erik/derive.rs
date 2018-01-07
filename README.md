# derive.rs

![demo](https://i.imgur.com/SXYasIX.gif)

Rust reimplementation of [derive](https://github.com/erik/derive).

```
# Unless you want to fill your drive with frame data, use a FIFO.
mkfifo heatmap-fifo.ppm

# In first terminal
cargo run --release --                                           \
          --bounds '34.205911 -119.009399 33.709276 -118.026123' \
          --width 2000                                           \
          --output heatmap.png                                   \
          --ppm-stream heatmap-fifo.ppm                          \
          --frame-rate 1950                                      \
          ~/Downloads/strava-data-dump/

# In second terminal
ffmpeg -i heatmap-fifo.ppm -y heatmap.mp4
```
