extern crate crossbeam;
extern crate image;
extern crate num;

use std::fs::File;
use std::io::Write;
use std::str::FromStr;

use image::{ColorType, ImageEncoder};
use image::codecs::png::PngEncoder;
use num::Complex;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 7 {
        writeln!(std::io::stderr(), "mandelbrot_set FILE THREADS LIMIT PIXELS UPPER_LEFT LOWER_RIGHT").unwrap();
        writeln!(std::io::stderr(), "example: mandelbrot_set pic.png 1500x750 -1.0,1.0 1.0,-1.0").unwrap();
        std::process::exit(1);
    }
    let filename = &args[1];
    let threads = u32::from_str(&args[2]).expect("expected THREADS: 8");
    let limit = u32::from_str(&args[3]).expect("expected LIMIT: 100");
    let bounds = parse_pair::<usize>(&args[4], "x").expect("expected PIXELS: 1500x750");
    let upper_left = parse_complex(&args[5]).expect("expected UPPER_LEFT: -1.0,1.25");
    let lower_right = parse_complex(&args[6]).expect("expected LOWER_RIGHT: 1.0,-1.0");

    let mut pixels: Vec<u8> = vec![0; bounds.0 * bounds.1];
    let rows_per_band = bounds.1 / (threads as usize) + 1;
    let bands: Vec<&mut [u8]> = pixels.chunks_mut(rows_per_band * bounds.0).collect();
    crossbeam::scope(|spawner| {
        for (i, band) in bands.into_iter().enumerate() {
            let top = rows_per_band * i;
            let height = band.len() / bounds.0;
            let band_bound = (bounds.0, height);
            let band_upper_left = pixel_to_point(bounds, (0, top), upper_left, lower_right);
            let band_lower_right = pixel_to_point(bounds, (bounds.0, top + height), upper_left, lower_right);
            spawner.spawn(move |_| {
                render(limit, band, band_bound, band_upper_left, band_lower_right);
            });
        }
    }).expect("crossbeam::scope panic");
    write_image(&filename, &pixels, bounds)
}

fn write_image(filename: &str, pixels: &[u8], bounds: (usize, usize)) {
    let output = File::create(filename).expect("File::create error");
    let encoder = PngEncoder::new(output);
    encoder.write_image(&pixels, bounds.0 as u32, bounds.1 as u32, ColorType::L8)
        .expect("write_image error")
}

/// Renders Mandelbrot set on the complex plane with borders `upper_left` and `lower_right`
/// for an image of size `bounds(width, height)`.
///
/// If point belongs color is black, otherwise white.
///
/// `limit` - amount of iterations for check whether point belongs to set or not.
fn render(limit: u32, pixels: &mut [u8], bounds: (usize, usize), upper_left: Complex<f64>, lower_right: Complex<f64>) {
    assert_eq!(pixels.len(), bounds.0 * bounds.1);
    for row in 0..bounds.1 {
        for column in 0..bounds.0 {
            let point = pixel_to_point(bounds, (column, row), upper_left, lower_right);
            pixels[row * bounds.0 + column] = match belongs_to_mandelbrot_set(point, limit) {
                None => 0,
                Some(_) => 255
            }
        }
    }
}

/// For limit operations checks whether `c` belongs to Mandelbrot set.
/// If definitely not returns `Some(iterations_count)`.
/// If possibly yes return `None`.
fn belongs_to_mandelbrot_set(c: Complex<f64>, limit: u32) -> Option<u32> {
    let mut z = Complex { re: 0.0, im: 0.0 };
    // iterate until limit
    for i in 0..limit {
        z = z * z + c;
        // if z^2 > 4 => z tends to infinity and c doesn't belong to Mandelbrot set
        // i.e. modulus > 2 in the complex plane
        if z.norm_sqr() > 4.0 {
            return Some(i);
        }
    }
    None
}

/// Maps pixel from image with size `bounds` to point on the complex plane with borders `upper_left` and `lower_right`.
///
/// `bounds` - width and height of an image in pixels.
/// `pixel` - coordinates of a pixel on an image with `bounds`.
/// `upper_left` - point on the complex plane corresponds to upper left corner of an image.
/// `lower_right` - point on the complex plane corresponds to lower right corner of an image.
fn pixel_to_point(bounds: (usize, usize), pixel: (usize, usize), upper_left: Complex<f64>, lower_right: Complex<f64>) -> Complex<f64> {
    let (complex_width, complex_height) = (lower_right.re - upper_left.re, upper_left.im - lower_right.im);
    Complex {
        re: upper_left.re + pixel.0 as f64 / bounds.0 as f64 * complex_width,
        im: upper_left.im - pixel.1 as f64 / bounds.1 as f64 * complex_height,
    }
}

///                25
///                         Im
///                         ^
///                         . 1
///                         .
///                         .
///                         .
///              -0.5       .
///          -------*---------------------> Re
///                 .       .             1
///                 .       .
///     75          * - - - . -0.5
///                         .
///                         .
///                         .
#[test]
fn test_pixel_to_point() {
    assert_eq!(
        pixel_to_point(
            (100, 100),
            (25, 75),
            Complex { re: -1.0, im: 1.0 },
            Complex { re: 1.0, im: -1.0 },
        ),
        Complex { re: -0.5, im: -0.5 }
    )
}

/// Parses comma separated complex number, e.g. `"1.25,0.42"` => Complex {re: 1.25, im: 0.42}.
fn parse_complex<T: FromStr>(s: &str) -> Option<Complex<T>> {
    match parse_pair::<T>(s, ",") {
        Some((re, im)) => Some(Complex { re, im }),
        None => None
    }
}

#[test]
fn test_parse_complex() {
    assert_eq!(parse_complex::<f64>("1.25,-0.42"), Some(Complex { re: 1.25, im: -0.42 }));
    assert_eq!(parse_complex::<u32>("125,42"), Some(Complex { re: 125, im: 42 }));
    assert_eq!(parse_complex::<u32>("1.25xb"), None);
}

/// Parses string `s` with pare of type T separated by `separator`, e.g. `"1.25x0.42"` => (1.25, 0.42).
fn parse_pair<T: FromStr>(s: &str, separator: &str) -> Option<(T, T)> {
    match s.find(separator) {
        None => None,
        Some(index) => {
            match (T::from_str(&s[0..index]), T::from_str(&s[index + 1..])) {
                (Ok(l), Ok(r)) => Some((l, r)),
                _ => None
            }
        }
    }
}

#[test]
fn test_parse_pair() {
    assert_eq!(parse_pair::<f64>("1.25x0.42", "x"), Some((1.25, 0.42)));
    assert_eq!(parse_pair::<i32>("125#42", "#"), Some((125, 42)));
    assert_eq!(parse_pair::<i8>("a,b", ","), None);
    assert_eq!(parse_pair::<i8>("ab", ","), None);
}