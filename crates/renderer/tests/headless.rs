use foundation::{Camera, Mat4, Transform, Vec3, Vec4};
use graphics::{Gpu, GpuConfig};
use renderer::{
    CameraData, InstanceData, MaterialData, RenderScene, Renderer, RendererConfig, checker_texture,
    cube, plane, uv_sphere,
};
use wgpu::TextureFormat;

fn demo_scene() -> RenderScene {
    let mut scene = RenderScene::default();
    let checker = scene.add_texture(checker_texture(
        256,
        32,
        [220, 220, 220, 255],
        [90, 90, 90, 255],
    ));
    let ground_mesh = scene.add_mesh(plane(40.0, 8));
    let cube_mesh = scene.add_mesh(cube(1.0));
    let sphere_mesh = scene.add_mesh(uv_sphere(0.55, 32, 16));
    let red = scene.add_material(MaterialData::pbr(Vec4::new(0.9, 0.15, 0.1, 1.0), 0.0, 0.35));
    let metal = scene.add_material(MaterialData::pbr(Vec4::new(0.9, 0.9, 0.95, 1.0), 1.0, 0.18));
    let ground = scene.add_material(
        MaterialData::pbr(Vec4::splat(1.0), 0.0, 0.9)
            .with_textures(checker, 1, 2, 3)
            .with_uv_scale([8.0, 8.0], [0.0, 0.0]),
    );
    let ground_bounds = scene.meshes[ground_mesh as usize].bounds;

    let cube_bounds = scene.meshes[cube_mesh as usize].bounds;
    let sphere_bounds = scene.meshes[sphere_mesh as usize].bounds;

    let cube_model = Transform::from_translation(Vec3::new(-1.2, 0.5, 0.0)).matrix();
    let sphere_model = Transform::from_translation(Vec3::new(1.1, 0.6, 0.0)).matrix();
    scene.instances.push(InstanceData::new(
        Mat4::IDENTITY,
        &ground_bounds,
        ground_mesh,
        ground,
    ));
    scene
        .instances
        .push(InstanceData::new(cube_model, &cube_bounds, cube_mesh, red));
    scene.instances.push(InstanceData::new(
        sphere_model,
        &sphere_bounds,
        sphere_mesh,
        metal,
    ));

    let camera = Camera::perspective(
        &Transform::looking_at(Vec3::new(0.0, 2.0, 6.0), Vec3::ZERO, Vec3::Y),
        50f32.to_radians(),
        16.0 / 9.0,
        0.1,
    );
    scene.camera = CameraData::from_camera(&camera);
    scene.settings.exposure = 0.75;
    scene.sun.intensity = 4.0;
    scene
}

#[test]
fn headless_render_produces_image() {
    let _ = env_logger::builder().is_test(true).try_init();
    let gpu = match Gpu::new(&GpuConfig {
        features: RendererConfig::required_features(),
        ..Default::default()
    }) {
        Ok(gpu) => gpu,
        Err(err) => {
            eprintln!("no GPU available, skipping test: {err}");
            return;
        }
    };

    let scene = demo_scene();
    let mut renderer = Renderer::new(
        &gpu,
        RendererConfig::default(),
        TextureFormat::Rgba8UnormSrgb,
    );
    let stats = renderer.render_offscreen(&scene, 640, 360);
    assert_eq!(stats.instances, 3);

    let pixels = renderer
        .read_offscreen()
        .expect("failed to read back pixels");
    assert_eq!(pixels.len(), 640 * 360 * 4);

    let corner = &pixels[0..4];
    let center = &pixels[((180 * 640 + 320) * 4) as usize..((180 * 640 + 320) * 4 + 4) as usize];
    let corner_luma = corner[0] as u32 + corner[1] as u32 + corner[2] as u32;
    let center_luma = center[0] as u32 + center[1] as u32 + center[2] as u32;
    assert!(corner_luma > 30, "sky is too dark: {corner:?}");
    assert!(
        center_luma > 10,
        "no geometry rendered in the center: {center:?}"
    );
    assert!(
        corner_luma.abs_diff(center_luma) > 3,
        "geometry matches the sky, nothing was rendered"
    );

    if let Ok(path) = std::env::var("FLUX_SCREENSHOT") {
        image::RgbaImage::from_raw(640, 360, pixels)
            .expect("unexpected pixel dimensions")
            .save(&path)
            .expect("failed to save screenshot");
        println!("screenshot: {path}");
    }

    println!(
        "GPU: {} | instances {} | draws {} | gpu culling {}",
        gpu.info.name, stats.instances, stats.draw_calls, stats.gpu_driven
    );
}
