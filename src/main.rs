#![allow(incomplete_features)]
#![feature(adt_const_params)]
use std::{
    error::Error,
    f64::consts::PI,
    fs::read_dir,
    io::{self, Write},
    ops::Range,
    path::{self, Path, PathBuf},
    time::Instant,
};

use colortypes::{CIELcha, Color, Image, Rgba, SRGB};
use image::{imageops::FilterType::Lanczos3, ImageBuffer};
use indicatif::ProgressBar;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

#[allow(dead_code)]
enum FindType {
    File,
    Dir,
}

fn list_dir<P: AsRef<Path>>(dir: P, find_dirs: FindType) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::<PathBuf>::new();
    for item in read_dir(dir)? {
        let item = item?;
        match &find_dirs {
            FindType::File => {
                if item.file_type()?.is_file() {
                    files.push(item.path());
                }
            }
            FindType::Dir => {
                if item.file_type()?.is_dir() {
                    files.push(item.path());
                }
            }
        }
    }
    Ok(files)
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct Label {
    color: Color<Rgba, SRGB>,
    image: Image<Rgba, SRGB>,
    o: Image<Rgba, SRGB>,
    m: Image<Rgba, SRGB>,
    count: u64,
    id: usize,
}

#[inline(always)]
fn get_palette_dimensions<P: AsRef<Path>>(pname: P) -> Result<(usize, usize), Box<dyn Error>> {
    let files = list_dir(&pname, FindType::File)?;
    let image = image::open(&files[0]).unwrap().into_rgba8();
    Ok((image.width() as usize, image.height() as usize))
}

#[inline(always)]
fn resize_dims((mut w, mut h): (usize, usize), max_size: u32) -> (usize, usize) {
    let max_dimension = w.max(h) as f32;
    w = (((w as f32) / max_dimension) * (max_size as f32)) as usize;
    h = (((h as f32) / max_dimension) * (max_size as f32)) as usize;
    (w, h)
}

#[inline(always)]
fn dynamic_to_image(
    input: &ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    size: (usize, usize),
) -> Image<Rgba, SRGB> {
    let image = input.to_vec();
    let mut new_img = vec![Rgba::new([0.0; 4]); 0];
    for slice in image.chunks_exact(4) {
        if let [r, g, b, a] = *slice {
            new_img.push(Rgba::new::<SRGB>([
                r as f64 / 256.0,
                g as f64 / 256.0,
                b as f64 / 256.0,
                a as f64 / 256.0,
            ]))
        };
    }
    Image::from_vec(size, new_img)
}

#[inline(always)]
fn vec_f64_to_image(input: &[f64], size: (usize, usize)) -> Image<Rgba, SRGB> {
    let mut new_img = vec![Rgba::new([0.0; 4]); 0];
    for slice in input.chunks_exact(4) {
        if let [r, g, b, a] = *slice {
            new_img.push(Rgba::new::<SRGB>([
                r as f64 / 256.0,
                g as f64 / 256.0,
                b as f64 / 256.0,
                a as f64 / 256.0,
            ]))
        };
    }
    Image::from_vec(size, new_img)
}

// Creates a Vec of the struct Label, which houses every
//      single image in the palette, with its computed color
fn generate_pixel_palette<P: AsRef<Path>>(
    pname: P,
    max_size: u32,
) -> Result<Vec<Label>, Box<dyn Error>> {
    let (pw, ph) = resize_dims(get_palette_dimensions(&pname)?, max_size);

    let set_count: usize = read_dir(&pname).unwrap().count() as usize;
    let mut palette = vec![
        Label {
            color: Rgba::new([0.0; 4]),
            image: Image::new((pw, ph)),
            o: Image::new((pw, ph)),
            m: Image::new((pw, ph)),
            count: 0,
            id: 0
        };
        set_count
    ];

    let mut tempfix = 0;
    let files = list_dir(&pname, FindType::File)?;
    let c = files.len();
    let bar = ProgressBar::new(c as u64);

    println!("Assembling palette in memory");

    for (i, item) in files.iter().enumerate() {
        bar.inc(1);
        if item.as_os_str().to_str().unwrap().contains("ini") {
            tempfix += 1;
        } else {
            let image = image::open(&item)?
                .resize(pw as u32, ph as u32, Lanczos3)
                .into_rgba8();
            palette[i].image = dynamic_to_image(&image, (pw, ph));
            palette[i].o = get_orientation(&palette[i].image);
            palette[i].o = get_edges_simple(&palette[i].image);
            palette[i].color = palette[i].image.mean();
            palette[i].id = i - tempfix;
        }
    }
    bar.finish();
    Ok(palette)
}

fn get_edges_simple(window: &Image<Rgba, SRGB>) -> Image<Rgba, SRGB> {
    let mut m = window.clone();
    let (w, h) = (window.width(), window.height());
    for y in 0..h {
        for x in 0..w {
            m.put_pixel(
                (x, y),
                Rgba::new([0f64; 4])
                    - if x > 0 {
                        window.get_pixel((x - 1, y))
                    } else {
                        window.get_pixel((x, y))
                    }
                    - if x < w - 1 {
                        window.get_pixel((x + 1, y))
                    } else {
                        window.get_pixel((x, y))
                    }
                    - if y > 0 {
                        window.get_pixel((x, y - 1))
                    } else {
                        window.get_pixel((x, y))
                    }
                    - if y < h - 1 {
                        window.get_pixel((x, y + 1))
                    } else {
                        window.get_pixel((x, y))
                    }
                    + (m.get_pixel((x, y)) * 4.0),
            );
        }
    }
    m
}

fn get_orientation(window: &Image<Rgba, SRGB>) -> Image<Rgba, SRGB> {
    let mut o = window.clone();
    let (w, h) = (window.width(), window.height());
    for y in 0..h {
        for x in 0..w {
            o.put_pixel(
                (x, y),
                Rgba::new([1.0, 1.0, 1.0, 0.0])
                    - (((if y < h - 1 {
                        window.get_pixel((x, y + 1))
                    } else {
                        window.get_pixel((x, y))
                    } - if y > 0 {
                        window.get_pixel((x, y - 1))
                    } else {
                        window.get_pixel((x, y))
                    })
                    .atan2_color(
                        if x < w - 1 {
                            window.get_pixel((x + 1, y))
                        } else {
                            window.get_pixel((x, y))
                        } - if x > 0 {
                            window.get_pixel((x - 1, y))
                        } else {
                            window.get_pixel((x, y))
                        },
                    )) / (PI / 2.0)),
            );
        }
    }
    o
}

use colortypes::{
    impl_colorspace, impl_conversion, ColorGamut, ColorType, FromColorType, Rgba as CRgba,
};
impl_colorspace!(Drgba<SRGB>);
impl_conversion!(CRgba, Drgba, |color| { color.to_arr().0 });

fn find_closest_image<'a>(
    window: &'a Image<Rgba, SRGB>,
    palette: &'a Vec<Label>,
    scale: f32,
) -> &'a Label {
    let self_o = get_orientation(window);
    let self_m = get_edges_simple(window);

    let (w, h) = window.size;
    let c = Complex::new((w / 2) as f32, (h / 2) as f32);
    let mut w_inner = Image::<Rgba, SRGB>::new_with((w, h), Rgba::new([1.0, 1.0, 1.0, 1.0]));
    let mut w_outer = Image::<Rgba, SRGB>::new_with((w, h), Rgba::new([1.0, 1.0, 1.0, 1.0]));

    for (i, (inn, out)) in w_outer
        .pixels_mut()
        .iter_mut()
        .zip(w_inner.pixels_mut().iter_mut())
        .enumerate()
    {
        let x = i % w;
        let y = i / w;
        let p = Complex::new(x as f32, y as f32);
        let d = (c - p).norm().abs();
        *inn *= (d) as f64;
        *out *= ((w as f32).hypot(h as f32) - d) as f64;
    }

    let self_o = self_o * w_inner.clone();
    let self_m = self_m * w_inner.clone();

    let scaled_window = window.clone() * w_outer.clone();

    palette
        .par_iter()
        .max_by_key(|x| {
            let metric_o = self_o.ssim_in_space::<Drgba>((*x).o.clone() * w_inner.clone());
            let metric_m = self_m.ssim_in_space::<Drgba>((*x).m.clone() * w_inner.clone());

            let ssim = scaled_window.ssim((*x).image.clone() * w_outer.clone());

            let cov_in = window.covariance_in_space::<CIELcha>(&((*x).image));

            (((metric_o.0 * metric_m.0 + ssim.0 * ssim.0)
                * (metric_m.1 * metric_o.1).sqrt()
                * (metric_m.2 * metric_o.2).sqrt())
            .powf(scale as f64)
                * (ssim.0 * ssim.1 * ssim.2).powf(1.0 / scale as f64))
            .abs()
            .to_bits()
                + (1.0 - cov_in.0).abs().to_bits()
        })
        .unwrap()
}

// Maps pixels to palette items
fn generate_image_pixel_mode<P: AsRef<Path>>(
    image_path: P,
    pname: P,
    palette: &mut Vec<Label>,
    pmax: u32,
    imax: u32,
) -> Result<ImageBuffer<image::Rgba<u8>, Vec<u8>>, Box<dyn Error>> {
    let image = image::open(&image_path).unwrap();
    // Get dimensions of input image and palette items
    let (w, h) = resize_dims((image.width() as usize, image.height() as usize), imax);
    let (pw, ph) = resize_dims(get_palette_dimensions(&pname)?, pmax);
    let scale = pw as f32 / w as f32;
    // Resize input image
    let image = image.resize(imax, imax, Lanczos3).into_rgba8();
    let image = dynamic_to_image(&image, (imax as usize, imax as usize));
    let output = ImageBuffer::<image::Rgba<u8>, Vec<u8>>::new((pw * w) as u32, (ph * h) as u32);
    let mut output = dynamic_to_image(&output, (pw * w, ph * h));
    let y_r = (((h - ph + 1) as f32 / ph as f32).ceil() as usize);
    let x_r = (((w - pw + 1) as f32 / pw as f32).ceil() as usize);
    let bar = ProgressBar::new((x_r * y_r) as u64);

    println!("Computing Mosaic...");

    for y in 0..y_r {
        for x in 0..x_r {
            bar.inc(1);
            let window = image.crop((x * pw, y * ph), (pw, ph));
            let window_avg = window.mean();
            // Find the closest item in the palette that matches the pixel
            let pimage = &find_closest_image(&window, palette, scale).image;

            // This part writes the pixels from the palette items to the output image
            for oy in 0..ph * ph {
                for ox in 0..pw * pw {
                    let mut px = pimage.get_pixel((ox / pw, oy / ph));
                    if px.3 < 0.5 {
                        let t = (window_avg.0 + window_avg.1 + window_avg.2) / 3.0;
                        px = Color::new([t; 4]);
                    }
                    px.3 = window_avg.3;
                    output.put_pixel((x * (pw * pw) + ox, y * (ph * ph) + oy), px);
                }
            }
        }
    }
    bar.finish();
    let dat = output.to_vec().iter().map(|x| (*x * 256.0) as u8).collect();
    Ok(
        ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw((pw * w) as u32, (ph * h) as u32, dat)
            .unwrap(),
    )
}

// #[allow(dead_code)]
// enum FindType {
//     File,
//     Dir,
// }

// fn list_dir<P: AsRef<Path>>(dir: P, find_dirs: FindType) -> Result<Vec<PathBuf>, Box<dyn Error>> {
//     let mut files = Vec::<PathBuf>::new();
//     for item in fs::read_dir(dir)? {
//         let item = item?;
//         match &find_dirs {
//             FindType::File => {
//                 if item.file_type()?.is_file() {
//                     files.push(item.path());
//                 }
//             }
//             FindType::Dir => {
//                 if item.file_type()?.is_dir() {
//                     files.push(item.path());
//                 }
//             }
//         }
//     }
//     Ok(files)
// }

fn prompt_number(bounds: Range<u32>, message: &str, def: i32) -> Result<u32, Box<dyn Error>> {
    let stdin = io::stdin();
    let mut buffer = String::new();
    // Tell the user to enter a value within the bounds
    if message != "" {
        if def >= 0 {
            println!(
                "{} in the range [{}:{}] (default: {})",
                message,
                bounds.start,
                bounds.end - 1,
                def
            );
        } else {
            println!(
                "{} in the range [{}:{}]",
                message,
                bounds.start,
                bounds.end - 1
            );
        }
    }
    buffer.clear();
    // Keep prompting until the user passes a value within the bounds
    Ok(loop {
        stdin.read_line(&mut buffer)?;
        print!("\r\u{8}");
        io::stdout().flush().unwrap();
        if let Ok(value) = buffer.trim().parse() {
            if bounds.contains(&value) {
                break value;
            }
        } else if def >= 0 {
            print!("\r\u{8}");
            print!("{}\n", &def);
            io::stdout().flush().unwrap();
            break def as u32;
        }
        buffer.clear();
    })
}

fn input_prompt<P: AsRef<Path>>(
    dir: P,
    find_dirs: FindType,
    message: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    // Get files/dirs in dir
    let files = list_dir(&dir, find_dirs)?;
    // Inform the user that they will need to enter a value
    if message != "" {
        println!("{}", message);
    }
    // Enumerate the names of the files/dirs
    for (i, e) in files.iter().enumerate() {
        println!("{}: {}", i, e.display());
    }
    // This is the range of values they can pick
    let bound: Range<u32> = Range {
        start: 0,
        end: files.len() as u32,
    };
    // Return the path they picked
    Ok((&files[prompt_number(bound, "", -1)? as usize]).clone())
}
use clap::Parser;
use rustdct::num_complex::Complex;
#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(short = 't', default_value = "0")]
    threads: usize,
    #[clap(short = 'f', default_value = "")]
    fpath: PathBuf,
    #[clap(short = 'p', default_value = "")]
    ppath: PathBuf,
    #[clap(default_value = "0")]
    psize: u32,
    #[clap(default_value = "0")]
    fsize: u32,

    #[clap(short = 'y', long)]
    ask: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = Args::parse();
    args.ask = !(args.ask);

    // Thread count
    if args.threads == 0 {
        args.threads = if args.ask {
            prompt_number(
                Range {
                    start: 1,
                    end: num_cpus::get() as u32,
                },
                "\nEnter the number of threads to use\nChoose a value",
                ((num_cpus::get() as f32) * 0.3).ceil() as i32,
            )? as usize
        } else {
            ((num_cpus::get() as f32) * 0.3).ceil() as usize
        }
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()
        .unwrap();

    // File name
    if String::from(args.fpath.to_string_lossy()) == "" {
        args.fpath = if args.ask {
            input_prompt("input", FindType::File, "\nChoose the input image")?
        } else {
            PathBuf::from(String::from("input/test.jpg"))
        }
    }
    let ps = args.fpath.file_name().unwrap().to_string_lossy();
    let fname = String::from(ps.split(".").collect::<Vec<&str>>()[0]);
    let ext = String::from(ps.split(".").collect::<Vec<&str>>()[1]);

    // Palette name
    if String::from(args.ppath.to_string_lossy()) == "" {
        args.ppath = if args.ask {
            input_prompt("palettes", FindType::Dir, "\nChoose the palette")?
        } else {
            PathBuf::from(String::from("palettes/emoji"))
        }
    }
    let ps = args.ppath.file_name().unwrap().to_string_lossy();
    let pname = String::from(ps.split(".").collect::<Vec<&str>>()[0]);

    // Palette size
    if args.psize == 0 {
        args.psize = if args.ask {
            prompt_number(
                Range { start: 4, end: 128 },
                "\nEnter a palette size\nChoose a value",
                16,
            )?
        } else {
            16
        }
    }

    // Image size
    let fmax = u32::MAX / args.psize;
    if args.fsize < args.psize || args.fsize > fmax || args.fsize == 0 {
        args.fsize = if args.ask {
            prompt_number(
                Range {
                    start: args.psize,
                    end: fmax,
                },
                "\nEnter a image size\nChoose a value",
                (8192 / args.psize) as i32,
            )?
        } else {
            8192 / args.psize
        }
    }

    // Output name
    let stdin = io::stdin();
    let mut buffer = String::new();
    println!("\nPlease enter the output name");
    stdin.read_line(&mut buffer)?;
    let outname = buffer.trim();

    let temp: Vec<&str> = args
        .ppath
        .as_os_str()
        .to_str()
        .unwrap()
        .split(path::MAIN_SEPARATOR)
        .collect();

    let mut name = format!(
        "output/{}_{}_f{}-p{}_{i}.{ext}",
        outname,
        temp[temp.len() - 1],
        args.fsize,
        args.psize,
        ext = ext,
        i = 0
    );

    let mut i = 1;
    let mut p = Path::new(&name);
    while p.exists() {
        name = format!(
            "output/{}_{}_f{}-p{}_{i}.{ext}",
            outname,
            temp[temp.len() - 1],
            args.fsize,
            args.psize,
            i = i,
            ext = ext,
        );
        i += 1;
        p = Path::new(&name);
    }

    println!(
        "Processing: {}.{} to {}, with palette: {}, at img size: {}, and palette size: {}",
        &fname, &ext, &name, &pname, &args.fsize, &args.psize
    );

    let pixel_mode = true;
    let now = Instant::now();
    if pixel_mode {
        let mut palette = generate_pixel_palette(&args.ppath, args.psize)?;
        let image = generate_image_pixel_mode(
            &args.fpath,
            &args.ppath,
            &mut palette,
            args.psize,
            args.fsize,
        )?;
        image.save(name)?;
    } else {
        // let palette = generate_mosaic_palette(pname, psize);
    }
    println!("Finished in: {}ms", now.elapsed().as_millis());
    Ok(())
}
