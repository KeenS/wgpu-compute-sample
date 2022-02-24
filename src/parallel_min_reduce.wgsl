let blockSize = 64u;
@group(0) @binding(0)
var<storage, read_write>  gmin: array<atomic<u32>>;

@stage(compute)
@workgroup_size(64) // @workgroup_size(blockSize)
fn reduce(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  atomicMin(&gmin[0], gmin[global_id[0]]);
}
