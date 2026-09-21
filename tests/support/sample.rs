/// A blobby shape with dark linework, close enough to the clipart these
/// cutters are usually traced from.
fn sample_image() -> RgbaImage {
    let (w, h) = (500u32, 500u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([210, 210, 210, 255]));
    let body = Rgba([214, 186, 138, 255]);
    let line = Rgba([70, 45, 35, 255]);
    let inside = |x: f32, y: f32| {
        let (nx, ny) = ((x - 250.0) / 180.0, (y - 260.0) / 190.0);
        nx * nx + ny * ny <= 1.0
            || ((x - 140.0).powi(2) / 3600.0 + (y - 120.0).powi(2) / 8100.0 <= 1.0)
            || ((x - 360.0).powi(2) / 3600.0 + (y - 120.0).powi(2) / 8100.0 <= 1.0)
    };
    for y in 0..h {
        for x in 0..w {
            if inside(x as f32, y as f32) {
                img.put_pixel(x, y, body);
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32, y as f32);
            if inside(fx, fy)
                && !(inside(fx - 4.0, fy)
                    && inside(fx + 4.0, fy)
                    && inside(fx, fy - 4.0)
                    && inside(fx, fy + 4.0))
            {
                img.put_pixel(x, y, line);
            }
        }
    }
    let mut seed = 12345u64;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as f32) / (u32::MAX as f32 / 2.0)
    };
    for _ in 0..50 {
        let cx = 90.0 + rnd() * 320.0;
        let cy = 110.0 + rnd() * 260.0;
        let r = 6.0 + rnd() * 8.0;
        for y in (cy - r) as u32..(cy + r) as u32 {
            for x in (cx - r) as u32..(cx + r) as u32 {
                if x < w && y < h {
                    let d = (x as f32 - cx).powi(2) + (y as f32 - cy).powi(2);
                    if d <= r * r && inside(x as f32, y as f32) {
                        img.put_pixel(x, y, line);
                    }
                }
            }
        }
    }
    img
}

