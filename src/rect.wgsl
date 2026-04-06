struct VSOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec4<f32>,

  @location(1) outer_min: vec2<f32>,
  @location(2) outer_max: vec2<f32>,
  @location(3) outer_radii: vec4<f32>,
  @location(4) inner_min: vec2<f32>,
  @location(5) inner_max: vec2<f32>,
  @location(6) inner_radii: vec4<f32>,
  @location(7) kind: f32,
};

@vertex
fn vs_main(
  @location(0) in_pos: vec2<f32>,
  @location(1) in_color: vec4<f32>,
  @location(2) in_outer_min: vec2<f32>,
  @location(3) in_outer_max: vec2<f32>,
  @location(4) in_outer_radii: vec4<f32>,
  @location(5) in_inner_min: vec2<f32>,
  @location(6) in_inner_max: vec2<f32>,
  @location(7) in_inner_radii: vec4<f32>,
  @location(8) in_kind: f32
) -> VSOut {
  var out: VSOut;
  out.pos = vec4<f32>(in_pos, 0.0, 1.0);
  out.color = in_color;

  out.outer_min = in_outer_min;
  out.outer_max = in_outer_max;
  out.outer_radii = in_outer_radii;
  out.inner_min = in_inner_min;
  out.inner_max = in_inner_max;
  out.inner_radii = in_inner_radii;
  out.kind = in_kind;

  return out;
}

fn point_in_rounded_rect(
  p: vec2<f32>,
  rect_min: vec2<f32>,
  rect_max: vec2<f32>,
  radii: vec4<f32>
) -> bool {
  if (rect_max.x <= rect_min.x || rect_max.y <= rect_min.y) {
    return false;
  }

  if (p.x < rect_min.x || p.x > rect_max.x || p.y < rect_min.y || p.y > rect_max.y) {
    return false;
  }

  let tl = max(radii.x, 0.0);
  let tr = max(radii.y, 0.0);
  let br = max(radii.z, 0.0);
  let bl = max(radii.w, 0.0);

  if (tl > 0.0 && p.x < rect_min.x + tl && p.y < rect_min.y + tl) {
    let delta = p - vec2<f32>(rect_min.x + tl, rect_min.y + tl);
    return dot(delta, delta) <= tl * tl;
  }

  if (tr > 0.0 && p.x > rect_max.x - tr && p.y < rect_min.y + tr) {
    let delta = p - vec2<f32>(rect_max.x - tr, rect_min.y + tr);
    return dot(delta, delta) <= tr * tr;
  }

  if (br > 0.0 && p.x > rect_max.x - br && p.y > rect_max.y - br) {
    let delta = p - vec2<f32>(rect_max.x - br, rect_max.y - br);
    return dot(delta, delta) <= br * br;
  }

  if (bl > 0.0 && p.x < rect_min.x + bl && p.y > rect_max.y - bl) {
    let delta = p - vec2<f32>(rect_min.x + bl, rect_max.y - bl);
    return dot(delta, delta) <= bl * bl;
  }

  return true;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  let p = in.pos.xy;

  if (!point_in_rounded_rect(p, in.outer_min, in.outer_max, in.outer_radii)) {
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
  }

  if (in.kind >= 0.5 && point_in_rounded_rect(p, in.inner_min, in.inner_max, in.inner_radii)) {
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
  }

  return in.color;
}
