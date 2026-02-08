struct VSOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) color: vec4<f32>,
};

@group(0) @binding(0)
var glyph_tex: texture_2d<f32>;

@group(0) @binding(1)
var glyph_samp: sampler;

@vertex
fn vs_main(@location(0) in_pos: vec2<f32>,
           @location(1) in_uv: vec2<f32>,
           @location(2) in_color: vec4<f32>) -> VSOut {
  var out: VSOut;
  out.pos = vec4<f32>(in_pos, 0.0, 1.0);
  out.uv = in_uv;
  out.color = in_color;
  return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  let a = textureSample(glyph_tex, glyph_samp, in.uv).r;
  return vec4<f32>(in.color.rgb, in.color.a * a);
}
