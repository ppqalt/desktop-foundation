# Overview backdrop

The normal wallpaper is drawn by swaybg. Niri's overview uses a separate prepared
blurred image behind workspace previews. The QML background layer ignores input
and exclusive zones.

Wallpaper changes publish the sharp image, overview image and configuration
together. The overview cache is keyed by image hash, scaling mode and blur policy.
Fill/fit preparation preserves aspect ratio and scales the longest edge to
1920 pixels before applying a 24-pixel Gaussian blur.

Use `scripts/wallpaper-set IMAGE` to change wallpaper and theme.
