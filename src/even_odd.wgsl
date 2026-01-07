@group(0) @binding(0)
var<storage, read> input: u32;

@group(0) @binding(1)
var<storage, read_write> output: u32;

@compute @workgroup_size(1)
fn is_even(@builtin(global_invocation_id) global_id: vec3<u32>) {
    const WORKGROUP_SIZE = 131072u;
    var known_even = global_id.x * WORKGROUP_SIZE;
    var count = WORKGROUP_SIZE / 2;
    for (; count > 1; count -= 1) {
        if (input == known_even) {
            output = 1;
        }
        known_even += 2;
    }
    if (input == known_even) {
        output = 1;
    }
}
