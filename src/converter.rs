use std::path::Path;
use std::{fs::File, io::Write};
use std::collections::HashMap;

use super::config::{Config, ConverterConfig, Hierarchical};
use super::svg::SvgFile;
use fastrand::Rng;
use visioncortex::color_clusters::{KeyingAction, Runner, RunnerConfig, HIERARCHICAL_MAX};
use visioncortex::{Color, ColorImage};

const NUM_UNUSED_COLOR_ITERATIONS: usize = 6;
/// The fraction of pixels in the top/bottom rows of the image that need to be transparent before
/// the entire image will be keyed.
const KEYING_THRESHOLD: f32 = 0.2;

/// Convert an in-memory image into an in-memory SVG
#[no_mangle]
pub extern "C" fn convert(img: ColorImage, config: Config) -> Result<SvgFile, String> {
    let config = config.into_converter_config();
    color_image_to_svg(img, config)
}

/// Convert an image file into svg file
#[no_mangle]
pub extern "C" fn convert_image_to_svg(
    input_path: &Path,
    output_path: &Path,
    config: Config,
) -> Result<(), String> {
    let img = read_image(input_path)?;
    let svg = convert(img, config)?;
    write_svg(svg, output_path)
}

#[no_mangle]
pub extern "C" fn color_exists_in_image(img: &ColorImage, color: Color) -> bool {
    for y in 0..img.height {
        for x in 0..img.width {
            let pixel_color = img.get_pixel(x, y);
            if pixel_color.r == color.r && pixel_color.g == color.g && pixel_color.b == color.b {
                return true;
            }
        }
    }
    false
}

#[no_mangle]
pub extern "C" fn find_unused_color_in_image(img: &ColorImage) -> Result<Color, String> {
    let special_colors = IntoIterator::into_iter([
        Color::new(255, 0, 0),
        Color::new(0, 255, 0),
        Color::new(0, 0, 255),
        Color::new(255, 255, 0),
        Color::new(0, 255, 255),
        Color::new(255, 0, 255),
    ]);
    let rng = Rng::new();
    let random_colors =
        (0..NUM_UNUSED_COLOR_ITERATIONS).map(|_| Color::new(rng.u8(..), rng.u8(..), rng.u8(..)));
    for color in special_colors.chain(random_colors) {
        if !color_exists_in_image(img, color) {
            return Ok(color);
        }
    }
    // TODO: Return a non keying mode of the image
    Err(String::from(
        "unable to find unused color in image to use as key",
    ))
}

#[no_mangle]
pub extern "C" fn should_key_image(img: &ColorImage) -> bool {
    if img.width == 0 || img.height == 0 {
        return false;
    }

    // Check for transparency at several scanlines
    let threshold = ((img.width * 2) as f32 * KEYING_THRESHOLD) as usize;
    let mut num_transparent_pixels = 0;
    let y_positions = [
        0,
        img.height / 4,
        img.height / 2,
        3 * img.height / 4,
        img.height - 1,
    ];
    for y in y_positions {
        for x in 0..img.width {
            if img.get_pixel(x, y).a == 0 {
                num_transparent_pixels += 1;
            }
            if num_transparent_pixels >= threshold {
                return true;
            }
        }
    }

    false
}

#[no_mangle]
pub extern "C" fn color_image_to_svg(mut img: ColorImage, config: ConverterConfig) -> Result<SvgFile, String> {
    let width = img.width;
    let height = img.height;

    // Apply K-means clustering to limit the number of colors
    let num_clusters = config.max_colors;
    let max_iterations = config.kmeans_max_iterations;
    let centroids = kmeans_clustering(&img, num_clusters, max_iterations);

    let key_color = if should_key_image(&img) {
        let key_color = find_unused_color_in_image(&img)?;
        key_color
    } else {
        Color::default()
    };

    // Map each pixel to the nearest centroid
    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            if pixel.a == 0 {
                img.set_pixel(x, y, &key_color);
            } else {    
                let closest_centroid = centroids
                    .iter()
                    .min_by_key(|&&centroid| color_distance(pixel, centroid))
                    .unwrap();
                img.set_pixel(x, y, closest_centroid);
            }
        }
    }

    let runner = Runner::new(
        RunnerConfig {
            diagonal: config.layer_difference == 0,
            hierarchical: HIERARCHICAL_MAX,
            batch_size: 25600,
            good_min_area: config.filter_speckle_area,
            good_max_area: (width * height),
            is_same_color_a: config.color_precision_loss,
            is_same_color_b: 1,
            deepen_diff: config.layer_difference,
            hollow_neighbours: 1,
            key_color,
            keying_action: if matches!(config.hierarchical, Hierarchical::Cutout) {
                KeyingAction::Keep
            } else {
                KeyingAction::Discard
            },
        },
        img,
    );

    let mut clusters = runner.run();

    match config.hierarchical {
        Hierarchical::Stacked => {}
        Hierarchical::Cutout => {
            let view = clusters.view();
            let image = view.to_color_image();
            let runner = Runner::new(
                RunnerConfig {
                    diagonal: false,
                    hierarchical: 64,
                    batch_size: 25600,
                    good_min_area: 0,
                    good_max_area: (image.width * image.height) as usize,
                    is_same_color_a: 0,
                    is_same_color_b: 1,
                    deepen_diff: 0,
                    hollow_neighbours: 0,
                    key_color,
                    keying_action: KeyingAction::Discard,
                },
                image,
            );
            clusters = runner.run();
        }
    }

    let view = clusters.view();

    let mut svg = SvgFile::new(width, height, config.path_precision);
    for &cluster_index in view.clusters_output.iter().rev() {
        let cluster = view.get_cluster(cluster_index);
        let paths = cluster.to_compound_path(
            &view,
            false,
            config.mode,
            config.corner_threshold,
            config.length_threshold,
            config.max_iterations,
            config.splice_threshold,
        );
        svg.add_path(paths, cluster.residue_color());
    }

    Ok(svg)
}

#[no_mangle]
pub extern "C" fn read_image(input_path: &Path) -> Result<ColorImage, String> {
    let img = image::open(input_path);
    let img = match img {
        Ok(file) => file.to_rgba8(),
        Err(_) => return Err(String::from("No image file found at specified input path")),
    };

    let (width, height) = (img.width() as usize, img.height() as usize);
    let img = ColorImage {
        pixels: img.as_raw().to_vec(),
        width,
        height,
    };

    Ok(img)
}

#[no_mangle]
pub extern "C" fn write_svg(svg: SvgFile, output_path: &Path) -> Result<(), String> {
    let out_file = File::create(output_path);
    let mut out_file = match out_file {
        Ok(file) => file,
        Err(_) => return Err(String::from("Cannot create output file.")),
    };

    write!(&mut out_file, "{}", svg).expect("failed to write file.");

    Ok(())
}

#[no_mangle]
pub extern "C" fn kmeans_clustering(img: &ColorImage, num_clusters: usize, max_iterations: usize) -> Vec<Color> {
    let rng = fastrand::Rng::new();
    let mut centroids: Vec<Color> = (0..num_clusters)
        .map(|_| {
            let x = rng.usize(..img.width);
            let y = rng.usize(..img.height);
            img.get_pixel(x, y)
        })
        .collect();

    for _ in 0..max_iterations {
        let mut clusters: HashMap<usize, Vec<Color>> = HashMap::new();

        for y in 0..img.height {
            for x in 0..img.width {
                let pixel = img.get_pixel(x, y);
                let closest_centroid = centroids
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, &centroid)| color_distance(pixel, centroid))
                    .map(|(index, _)| index)
                    .unwrap();

                clusters.entry(closest_centroid).or_default().push(pixel);
            }
        }

        for (i, cluster) in clusters.iter() {
            let sum = cluster.iter().fold((0, 0, 0), |acc, &color| {
                (acc.0 + color.r as usize, acc.1 + color.g as usize, acc.2 + color.b as usize)
            });
            let count = cluster.len() as usize;
            centroids[*i] = Color::new(
                (sum.0 / count) as u8,
                (sum.1 / count) as u8,
                (sum.2 / count) as u8,
            );
        }
    }

    centroids
}

#[no_mangle]
pub extern "C" fn color_distance(c1: Color, c2: Color) -> usize {
    let dr = c1.r as isize - c2.r as isize;
    let dg = c1.g as isize - c2.g as isize;
    let db = c1.b as isize - c2.b as isize;
    (dr * dr + dg * dg + db * db) as usize
}