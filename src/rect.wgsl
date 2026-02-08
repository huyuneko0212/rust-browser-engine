struct VSOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@location(0) in_pos: vec2<f32>,
           @location(1) in_color: vec4<f32>) -> VSOut {
  var out: VSOut;
  out.pos = vec4<f32>(in_pos, 0.0, 1.0);
  out.color = in_color;
  return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  return in.color;
}
