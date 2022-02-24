let blockSize = 64u;

@group(0) @binding(0)
var<storage>               src: array<vec4<u32>>;
@group(0) @binding(1)
var<storage, read_write>  gmin: array<atomic<u32>>;
var<workgroup>            lmin: array<atomic<u32>, blockSize>;
@group(0) @binding(2)
var<storage, read_write>   dbg: array<u32>;

var<push_constant> nitems: u32;

@stage(compute)
@workgroup_size(64) // @workgroup_size(blockSize)
fn minp(
   @builtin(global_invocation_id) global_id: vec3<u32>,
   @builtin(local_invocation_id) local_id: vec3<u32>,
   @builtin(workgroup_id) workgroup_id: vec3<u32>,
   @builtin(num_workgroups) global_size: vec3<u32>,
) {
  let count = (nitems / 4u) / global_size[0];
  var idx = global_id[0] * count;
  let stride = 1u;
  var pmin = bitcast<u32>(-1);
  for(var n = 0u; n < count; n += 1u)
  {
    pmin = min(pmin, src[idx].x);
    pmin = min(pmin, src[idx].y);
    pmin = min(pmin, src[idx].z);
    pmin = min(pmin, src[idx].w);
    idx += stride;
  }

  if local_id[0] == 0u {
    lmin[0] = bitcast<u32>(-1);
  }
  workgroupBarrier();
  atomicMin(&lmin[0], pmin);
  workgroupBarrier();

  if local_id[0] == 0u {
    gmin[workgroup_id[0]] = lmin[0];
  }
  if(global_id[0] == 0u) {
    dbg[0] = global_size[0] / blockSize;
    dbg[1] = global_size[0];
    dbg[2] = count;
    dbg[3] = stride;
    dbg[4] = nitems;
  }
}
