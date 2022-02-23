struct Array {
  a: array<f32>;
};

@group(0) @binding(0)
var<storage>             x: Array;
@group(0) @binding(1)
var<storage, read_write> y: Array;

struct PushConstant {
  a: f32;
};

var<push_constant> a: PushConstant;

@stage(compute) @workgroup_size(64)
fn cs_main(
   @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let start = global_id.x * 64u;
  for(var i = start; i < start + 64u; i += 1u) {
    y.a[i] = a.a * x.a[i] + y.a[i];
  }
}
