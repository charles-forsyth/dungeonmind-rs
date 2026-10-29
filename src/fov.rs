/// Field of view via Bresenham line-of-sight over a radius box.
/// `solid(x,y)` blocks sight (out-of-bounds should be reported solid by caller).
pub fn compute_fov(
    w: usize,
    h: usize,
    cx: i32,
    cy: i32,
    radius: i32,
    solid: &impl Fn(i32, i32) -> bool,
    mark: &mut impl FnMut(usize, usize),
) {
    mark(cx as usize, cy as usize);
    let r2 = radius * radius;
    for y in (cy - radius)..=(cy + radius) {
        for x in (cx - radius)..=(cx + radius) {
            if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
                continue;
            }
            let ddx = x - cx;
            let ddy = y - cy;
            if ddx * ddx + ddy * ddy > r2 {
                continue;
            }
            if has_los(cx, cy, x, y, solid) {
                mark(x as usize, y as usize);
            }
        }
    }
}

/// True if no solid tile strictly between (x0,y0) and (x1,y1) blocks the ray.
pub fn has_los(x0: i32, y0: i32, x1: i32, y1: i32, solid: &impl Fn(i32, i32) -> bool) -> bool {
    if x0 == x1 && y0 == y1 {
        return true;
    }
    let mut dx = (x1 - x0).abs();
    let mut dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        let e2 = 2 * err;
        let mut moved_diag = false;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
            if e2 >= dy {
                moved_diag = true;
            }
        }
        // corner-cut guard: diagonal step must not leak between two walls
        if moved_diag && solid(x - sx, y) && solid(x, y - sy) {
            return false;
        }
        if x == x1 && y == y1 {
            return true;
        }
        if solid(x, y) {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn walls_block() {
        let solid = |x: i32, y: i32| x == 2 && y == 0;
        assert!(has_los(0, 0, 1, 0, &solid));
        assert!(!has_los(0, 0, 3, 0, &solid));
        assert!(has_los(0, 0, 0, 3, &solid));
    }
    #[test]
    fn fov_marks_visible() {
        let solid = |x: i32, y: i32| x == 3 && y != 1;
        let mut vis = vec![];
        compute_fov(10, 10, 0, 1, 5, &solid, &mut |x, y| vis.push((x, y)));
        assert!(vis.contains(&(4, 1))); // through the gap
        assert!(!vis.contains(&(4, 3))); // behind wall
        assert!(vis.contains(&(0, 1))); // self
    }
    #[test]
    fn no_corner_leak() {
        let solid = |x: i32, y: i32| (x == 1 && y == 0) || (x == 0 && y == 1);
        assert!(!has_los(0, 0, 1, 1, &solid));
    }
}
