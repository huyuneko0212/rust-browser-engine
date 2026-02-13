struct VSOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) in_pos: vec2<f32>, @location(1) in_uv: vec2<f32>) -> VSOut {
  var o: VSOut;
  o.pos = vec4<f32>(in_pos, 0.0, 1.0);
  o.uv = in_uv;
  return o;
}

@group(0) @binding(0) var img_tex: texture_2d<f32>;
@group(0) @binding(1) var img_samp: sampler;

@fragment
fn fs_main(i: VSOut) -> @location(0) vec4<f32> {
  return textureSample(img_tex, img_samp, i.uv);
}
