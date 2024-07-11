use std::env::args;
use std::fs::File;
use std::path::Path;
use std::str::FromStr;

use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};
use num_complex::Complex;

struct AppArgs {
    target_file_name: Box<Path>,
    bounds: (usize, usize),
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
}

fn main() {
    let app_args: AppArgs = parse_app_args();
    let pixels = render_concurrent(app_args.bounds, app_args.upper_left, app_args.lower_right);

    write_image(&app_args.target_file_name, &pixels, app_args.bounds)
        .expect("Can't save result image.");
}

fn parse_app_args() -> AppArgs {
    let args: Vec<String> = args().skip(1).collect();
    if args.len() != 4 {
        eprintln!("Need 4 arguments: <TARGET_FILE_NAME_PNG> <BOUNDS> <UPPER_LEFT_COMPLEX_NUM_POINT> <LOWER_RIGHT_COMPLEX_NUM_POINT>");
        std::process::exit(1);
    }

    let target_file_name = Path::new(args[0].clone().as_str())
        .to_owned()
        .into_boxed_path();
    let bounds = parse_pair::<usize>(&args[1].clone(), 'x').expect("Can't parse bounds!"); // 1024x768
    let upper_left = parse_complex(&args[2].clone()).expect("Can't parse upper left point!"); // -1.0,1.0
    let lower_right = parse_complex(&args[3].clone()).expect("Can't parse lower right point!"); // 1.0,-1.0

    AppArgs {
        target_file_name,
        bounds,
        upper_left,
        lower_right,
    }
}

fn render_concurrent(
    bounds: (usize, usize),
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
) -> Vec<u8> {
    let mut pixels = vec![0; bounds.0 * bounds.1];

    let threads = 8;
    let rows_per_band = bounds.1 / threads + 1;
    let bands: Vec<&mut [u8]> = pixels.chunks_mut(rows_per_band * bounds.0).collect();

    crossbeam::scope(|spawner| {
        for (i, band) in bands.into_iter().enumerate() {
            let top = rows_per_band * i;
            let height = band.len() / bounds.0;
            let band_bounds = (bounds.0, height);
            let band_upper_left = pixel_to_point(bounds, (0, top), upper_left, lower_right);
            let band_lower_right =
                pixel_to_point(bounds, (bounds.0, top + height), upper_left, lower_right);

            spawner.spawn(move |_| {
                render(band_bounds, band, band_upper_left, band_lower_right);
            });
        }
    })
    .unwrap();

    pixels
}

fn escape_time(c: Complex<f64>, limit: usize) -> Option<usize> {
    let mut z = Complex { re: 0.0, im: 0.0 };
    for i in 0..limit {
        if z.norm_sqr() > 8.0 {
            return Some(i);
        }
        z = z * z + c;
    }

    None
}

fn write_image(
    filename: &Path,
    pixels: &[u8],
    bounds: (usize, usize),
) -> Result<(), std::io::Error> {
    let output = File::create(filename)?;
    let encoder = PngEncoder::new(output);
    encoder
        .write_image(pixels, bounds.0 as u32, bounds.1 as u32, ColorType::L8)
        .expect("Could not write PNG file");

    Ok(())
}

fn render(
    bounds: (usize, usize),
    pixel: &mut [u8],
    upper_left_corner: Complex<f64>,
    lower_right_corner: Complex<f64>,
) {
    assert_eq!(pixel.len(), bounds.0 * bounds.1);

    for row in 0..bounds.1 {
        for col in 0..bounds.0 {
            let point = pixel_to_point(bounds, (col, row), upper_left_corner, lower_right_corner);

            pixel[row * bounds.0 + col] = match escape_time(point, 255) {
                None => 0,
                Some(count) => 255 - count as u8,
            }
        }
    }
}

fn pixel_to_point(
    bounds: (usize, usize),
    pixel: (usize, usize),
    upper_left_corner: Complex<f64>,
    lower_right_corner: Complex<f64>,
) -> Complex<f64> {
    let (width, height) = (
        lower_right_corner.re - upper_left_corner.re,
        upper_left_corner.im - lower_right_corner.im,
    );

    Complex {
        re: upper_left_corner.re + (pixel.0 as f64 * (width / bounds.0 as f64)),
        im: upper_left_corner.im - (pixel.1 as f64 * (height / bounds.1 as f64)),
    }
}

fn parse_pair<T: FromStr>(value: &str, separator: char) -> Option<(T, T)> {
    match value.find(separator) {
        None => None,
        Some(index) => {
            match (
                T::from_str(&value[..index].trim()),
                T::from_str(&value[index + 1..].trim()),
            ) {
                (Ok(left_val), Ok(right_val)) => Some((left_val, right_val)),
                _ => {
                    println!(
                        "[DEBUG] Tried to parse, and failed: [{}] [{}]",
                        &value[..index].trim(),
                        &value[index + 1..].trim()
                    );
                    None
                }
            }
        }
    }
}

fn parse_complex(value: &str) -> Option<Complex<f64>> {
    match parse_pair(value, ',') {
        Some((re, im)) => Some(Complex { re, im }),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_complex() {
        assert_eq!(
            parse_complex("0.12,0.1"),
            Some(Complex { re: 0.12, im: 0.1 })
        );

        assert_eq!(
            parse_complex("-0.12,-0.1"),
            Some(Complex {
                re: -0.12,
                im: -0.1
            })
        );
    }

    #[test]
    fn test_parse_pair() {
        assert_eq!(parse_pair::<u32>("", ','), None);
        assert_eq!(parse_pair::<u32>("123,0", ','), Some((123, 0)));
        assert_eq!(parse_pair::<f64>("0.12,0.1", ','), Some((0.12, 0.1)));
    }

    #[test]
    fn test_pixel_to_point() {
        assert_eq!(
            pixel_to_point(
                (100, 200),
                (25, 75),
                Complex { re: -1.0, im: 1.0 },
                Complex { re: 1.0, im: -1.0 }
            ),
            Complex { re: -0.5, im: 0.25 }
        );
    }
}
