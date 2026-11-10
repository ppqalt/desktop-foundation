#language slang 2026

import kitty_custom_shader_types;

static const float RADIUS = 14.0;
static const float BORDER_WIDTH = 1.0;
static const float BORDER_R = 0.0;
static const float BORDER_G = 0.0;
static const float BORDER_B = 0.0;

// Only the inward corners are rounded; the outer edge fills the output boundary.
public float4 fragment_main(float4 color, KittyTextures t, KittyCustomShaderData d) {
    float2 size = float2(d.viewport_size_pixels);
    float radius = min(RADIUS, min(size.x, size.y) * 0.5);
    float2 position = t.pos * size - size * 0.5;
    float2 q = abs(position) - (size * 0.5 - radius);
    float distance = t.pos.x < 0.5
        ? length(max(q, float2(0.0))) + min(max(q.x, q.y), 0.0) - radius
        : max(abs(position.x) - size.x * 0.5, abs(position.y) - size.y * 0.5);
    float outline = 1.0 - smoothstep(BORDER_WIDTH - 0.5, BORDER_WIDTH + 0.5, -distance);
    color.rgb = lerp(color.rgb, float3(BORDER_R, BORDER_G, BORDER_B), outline * 0.65);
    color.a = max(color.a, outline * 0.65);
    color.a *= 1.0 - smoothstep(-0.5, 0.5, distance);
    return color;
}
