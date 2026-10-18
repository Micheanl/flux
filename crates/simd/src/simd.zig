const std = @import("std");

const lane = 8;
const V = @Vector(lane, f32);
const B = @Vector(lane, bool);

inline fn splat(v: f32) V {
    return @splat(v);
}

export fn frustum_cull(mask: [*]u8, aabbs: [*]const f32, planes: [*]const f32, count: u32) u32 {
    var visible: u32 = 0;
    var i: usize = 0;
    const n: usize = count;

    while (i + lane <= n) : (i += lane) {
        var minx: V = undefined;
        var miny: V = undefined;
        var minz: V = undefined;
        var maxx: V = undefined;
        var maxy: V = undefined;
        var maxz: V = undefined;

        inline for (0..lane) |k| {
            const base = (i + k) * 6;
            minx[k] = aabbs[base + 0];
            miny[k] = aabbs[base + 1];
            minz[k] = aabbs[base + 2];
            maxx[k] = aabbs[base + 3];
            maxy[k] = aabbs[base + 4];
            maxz[k] = aabbs[base + 5];
        }

        const cx = (minx + maxx) * splat(0.5);
        const cy = (miny + maxy) * splat(0.5);
        const cz = (minz + maxz) * splat(0.5);
        const ex = (maxx - minx) * splat(0.5);
        const ey = (maxy - miny) * splat(0.5);
        const ez = (maxz - minz) * splat(0.5);

        var alive: B = @splat(true);
        var p: usize = 0;
        while (p < 6) : (p += 1) {
            const base = p * 4;
            const nx = splat(planes[base + 0]);
            const ny = splat(planes[base + 1]);
            const nz = splat(planes[base + 2]);
            const d = splat(planes[base + 3]);
            const dist = nx * cx + ny * cy + nz * cz + d;
            const radius = @max(nx, -nx) * ex + @max(ny, -ny) * ey + @max(nz, -nz) * ez;
            alive = @select(bool, dist + radius >= splat(0.0), alive, @as(B, @splat(false)));
        }

        inline for (0..lane) |k| {
            mask[i + k] = if (alive[k]) 1 else 0;
            if (alive[k]) visible += 1;
        }
    }

    while (i < n) : (i += 1) {
        const base = i * 6;
        const min = V{ aabbs[base + 0], aabbs[base + 1], aabbs[base + 2], 0, 0, 0, 0, 0 };
        const max = V{ aabbs[base + 3], aabbs[base + 4], aabbs[base + 5], 0, 0, 0, 0, 0 };
        const center = (min + max) * splat(0.5);
        const extents = (max - min) * splat(0.5);
        var inside = true;
        var p: usize = 0;
        while (p < 6) : (p += 1) {
            const pb = p * 4;
            const nx = planes[pb + 0];
            const ny = planes[pb + 1];
            const nz = planes[pb + 2];
            const d = planes[pb + 3];
            const dist = nx * center[0] + ny * center[1] + nz * center[2] + d;
            const radius = @max(nx, -nx) * extents[0] + @max(ny, -ny) * extents[1] + @max(nz, -nz) * extents[2];
            if (dist + radius < 0) {
                inside = false;
                break;
            }
        }
        mask[i] = if (inside) 1 else 0;
        if (inside) visible += 1;
    }
    return visible;
}

export fn mat4_mul(out: [*]f32, a: [*]const f32, b: [*]const f32) void {
    const ax = [4]V{
        V{ a[0], a[1], a[2], a[3], 0, 0, 0, 0 },
        V{ a[4], a[5], a[6], a[7], 0, 0, 0, 0 },
        V{ a[8], a[9], a[10], a[11], 0, 0, 0, 0 },
        V{ a[12], a[13], a[14], a[15], 0, 0, 0, 0 },
    };
    var j: usize = 0;
    while (j < 4) : (j += 1) {
        const c = [4]f32{ b[j * 4 + 0], b[j * 4 + 1], b[j * 4 + 2], b[j * 4 + 3] };
        const col = @Vector(4, f32){
            ax[0][0] * c[0] + ax[1][0] * c[1] + ax[2][0] * c[2] + ax[3][0] * c[3],
            ax[0][1] * c[0] + ax[1][1] * c[1] + ax[2][1] * c[2] + ax[3][1] * c[3],
            ax[0][2] * c[0] + ax[1][2] * c[1] + ax[2][2] * c[2] + ax[3][2] * c[3],
            ax[0][3] * c[0] + ax[1][3] * c[1] + ax[2][3] * c[2] + ax[3][3] * c[3],
        };
        out[j * 4 + 0] = col[0];
        out[j * 4 + 1] = col[1];
        out[j * 4 + 2] = col[2];
        out[j * 4 + 3] = col[3];
    }
}

export fn aabb_transform(out: [*]f32, aabbs: [*]const f32, matrix: [*]const f32, count: u32) void {
    const rows = [4]@Vector(4, f32){
        .{ matrix[0], matrix[4], matrix[8], matrix[12] },
        .{ matrix[1], matrix[5], matrix[9], matrix[13] },
        .{ matrix[2], matrix[6], matrix[10], matrix[14] },
        .{ matrix[3], matrix[7], matrix[11], matrix[15] },
    };
    var i: usize = 0;
    while (i < count) : (i += 1) {
        const base = i * 6;
        const center = @Vector(4, f32){
            (aabbs[base + 0] + aabbs[base + 3]) * 0.5,
            (aabbs[base + 1] + aabbs[base + 4]) * 0.5,
            (aabbs[base + 2] + aabbs[base + 5]) * 0.5,
            1.0,
        };
        const extents = @Vector(4, f32){
            (aabbs[base + 3] - aabbs[base + 0]) * 0.5,
            (aabbs[base + 4] - aabbs[base + 1]) * 0.5,
            (aabbs[base + 5] - aabbs[base + 2]) * 0.5,
            0.0,
        };
        var world_center = @Vector(4, f32){ 0, 0, 0, 0 };
        var world_extents = @Vector(4, f32){ 0, 0, 0, 0 };
        inline for (0..4) |r| {
            const dot_c = rows[r][0] * center[0] + rows[r][1] * center[1] + rows[r][2] * center[2] + rows[r][3] * center[3];
            world_center[r] = dot_c;
            const ae = @Vector(4, f32){
                @max(rows[r][0], -rows[r][0]),
                @max(rows[r][1], -rows[r][1]),
                @max(rows[r][2], -rows[r][2]),
                @max(rows[r][3], -rows[r][3]),
            };
            world_extents[r] = ae[0] * extents[0] + ae[1] * extents[1] + ae[2] * extents[2];
        }
        out[base + 0] = world_center[0] - world_extents[0];
        out[base + 1] = world_center[1] - world_extents[1];
        out[base + 2] = world_center[2] - world_extents[2];
        out[base + 3] = world_center[0] + world_extents[0];
        out[base + 4] = world_center[1] + world_extents[1];
        out[base + 5] = world_center[2] + world_extents[2];
    }
}

export fn transform_points(out: [*]f32, matrix: [*]const f32, points: [*]const f32, count: u32) void {
    var i: usize = 0;
    while (i < count) : (i += 1) {
        const x = points[i * 3 + 0];
        const y = points[i * 3 + 1];
        const z = points[i * 3 + 2];
        inline for (0..4) |r| {
            out[i * 3 + r] = matrix[r] * x + matrix[4 + r] * y + matrix[8 + r] * z + matrix[12 + r];
        }
    }
}

export fn simd_width() u32 {
    return @intCast(std.simd.suggestVectorLength(f32) orelse 4);
}
