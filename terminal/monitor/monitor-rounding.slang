#language slang 2026

import kitty_custom_shader_types;

static const float RADIUS = 14.0;

// Clip the complete panel, including its background, without dimming glyphs.
public float4 fragment_main(float4 color, KittyTextures t, KittyCustomShaderData d) {
    float2 size = float2(d.viewport_size_pixels);
    float radius = min(RADIUS, min(size.x, size.y) * 0.5);
    float2 position = t.pos * size - size * 0.5;
    float2 q = abs(position) - (size * 0.5 - radius);
    float distance = length(max(q, float2(0.0)))
        + min(max(q.x, q.y), 0.0) - radius;
    color.a *= 1.0 - smoothstep(-0.5, 0.5, distance);
    return color;
}
