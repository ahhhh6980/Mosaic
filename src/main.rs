// Mosaic
// Main File
// (C) 2022 by Jacob (ahhhh6980@gmail.com)

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use image::{imageops::FilterType::Lanczos3, ImageBuffer, Rgba};
use ndarray::Array1;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::ops::Range;
use std::path::{self, PathBuf};
use std::time::Instant;
use std::{cmp::max, fs::read_dir, path::Path};

#[derive(Debug, Clone)]
struct Label {
    color: Array1<f32>,
    image: ImageBuffer<Rgba<u8>, Vec<u8>>,
    count: u64,
    id: usize,
}
// #[derive(Debug, Clone)]
// struct LabelMosaic {
//     color: Array3<f32>,
//     image: ImageBuffer<Rgba<u8>, Vec<u8>>,
// }

fn compute_average_color(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, w: usize, h: usize) -> Array1<f32> {
    let mut avg = [0f32; 4];
    // Sum the pixels in the image
    for (i, e) in img.iter().enumerate() {
        avg[i % 4] += *e as f32
    }
    // Normalize the sum to 256
    for i in 0..4 {
        avg[i] /= h as f32 * w as f32;
    }
    Array1::from(avg.to_vec())
}

// This is based off of perceptual color, but it's results have been ok so far
fn bad_color_distance(pixel_a: &Array1<f32>, pixel_b: &Array1<f32>, q: u64, qfactor: f32) -> u64 {
    let c1 = pixel_a / 256.0;
    let c2 = pixel_b / 256.0;
    let dc = &c2 - &c1;
    let r = (c1[0] + c2[0]) / 2.0;
    let dr = (2.0 + (r / 256.0)) * dc[0] * dc[0];
    let dg = 4.0 * dc[1] * dc[1];
    let db = (2.0 + ((255.0 - r) / 256.0)) * dc[2] * dc[2];
    if qfactor > 0.0 {
        ((dr + dg + db) * 1024.0 + ((q as f32) / qfactor)) as u64
    } else {
        ((dr + dg + db) * 1024.0) as u64
    }
}

fn find_closest_image(
    pixel: &Array1<f32>,
    palette: &Vec<Label>,
    qfactor: f32,
) -> (ImageBuffer<Rgba<u8>, Vec<u8>>, Array1<f32>, usize) {
    //let label = palette.par_iter().min_by_key(|x| bad_color_distance(&pixel, &x.color, &x.count)).unwrap();
    let label = palette
        .par_iter()
        .min_by_key(|x| bad_color_distance(&pixel, &x.color, x.count, qfactor))
        .unwrap();
    (label.image.clone(), label.color.clone(), label.id.clone())
}

fn get_palette_dimensions<P: AsRef<Path>>(pname: P) -> Result<(usize, usize), Box<dyn Error>> {
    let files = list_dir(&pname, FindType::File)?;
    let image = image::open(&files[0]).unwrap().into_rgba8();
    Ok((image.width() as usize, image.height() as usize))
}

fn resize_dims((mut w, mut h): (usize, usize), max_size: u32) -> (usize, usize) {
    let max_dimension = max(w, h) as f32;
    w = (((w as f32) / max_dimension) * (max_size as f32)) as usize;
    h = (((h as f32) / max_dimension) * (max_size as f32)) as usize;
    (w, h)
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
            color: Array1::<f32>::zeros(4),
            image: ImageBuffer::<Rgba<u8>, Vec<u8>>::new(pw as u32, ph as u32),
            count: 0,
            id: 0
        };
        set_count
    ];

    let mut tempfix = 0;
    for (i, item) in list_dir(&pname, FindType::File)?.iter().enumerate() {
        if item.as_os_str().to_str().unwrap().contains("ini") == true {
            tempfix += 1;
        } else {
            let image = image::open(&item)?
                .resize(pw as u32, ph as u32, Lanczos3)
                .into_rgba8();
            palette[i].color = compute_average_color(&image, pw, ph);
            palette[i].image = image;
            palette[i].id = i - tempfix;
        }
    }
    Ok(palette)
}

// Maps pixels to palette items
fn generate_image_pixel_mode<P: AsRef<Path>>(
    image_path: P,
    pname: P,
    palette: &mut Vec<Label>,
    pmax: u32,
    imax: u32,
    qfactor: f32,
) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, Box<dyn Error>> {
    let image = image::open(&image_path).unwrap();
    // Get dimensions of input image and palette items
    let (w, h) = resize_dims((image.width() as usize, image.height() as usize), imax);
    let (pw, ph) = resize_dims(get_palette_dimensions(&pname)?, pmax);
    // Resize input image
    let image = image.resize(imax, imax, Lanczos3).into_rgba8();
    let mut output = ImageBuffer::<Rgba<u8>, Vec<u8>>::new((pw * w) as u32, (ph * h) as u32);
    for y in 0..h {
        for x in 0..w {
            let temp = (image[(x as u32, y as u32)].0).to_vec();
            let pixel: Vec<f32> = temp.iter().map(|v| *v as f32).collect();
            // Convert our pixel into an ndarray so we can do math with it
            let pixel = Array1::<f32>::from(pixel);
            // Find the closest item in the palette that matches the pixel
            let (pimage, pcolor, pid) = find_closest_image(&pixel, palette, qfactor);
            // Increment counter of found item, this is used for variance
            //  if variance is set high, it will include that value in distance calc
            palette[pid].count += 1;
            // This part writes the pixels from the palette items to the output image
            for oy in 0..ph {
                for ox in 0..pw {
                    let mut ppixel = pimage[(ox as u32, oy as u32)];
                    // If theres no transparency in the input, this
                    //  replaces the transparency in the palette
                    //  here with the palette items average value
                    if ppixel[3] < 128 {
                        let value: u16 = pcolor.iter().map(|v| *v as u16).sum();
                        ppixel[0] = (value / 4) as u8;
                        ppixel[1] = (value / 4) as u8;
                        ppixel[2] = (value / 4) as u8;
                    }
                    ppixel[3] = pixel[3] as u8;
                    output.put_pixel((x * pw + ox) as u32, (y * ph + oy) as u32, ppixel);
                }
            }
        }
    }
    Ok(output)
}

#[allow(dead_code)]
enum FindType {
    File,
    Dir,
}

fn list_dir<P: AsRef<Path>>(dir: P, find_dirs: FindType) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::<PathBuf>::new();
    for item in fs::read_dir(dir)? {
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
    #[clap(short = 'q', default_value = "-1")]
    qfactor: i32,
    #[clap(short = 'y', long)]
    ask: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = Args::parse();
    args.ask = !(args.ask);

    // Thread count
    if args.threads == 0 {
        args.threads = if args.ask {
            prompt_number(Range { start: 1, end: num_cpus::get() as u32 }, "\nEnter the number of threads to use\nOnly choose more than a couple for very large fsize values\n(like fsize over 512 w/ psize over 32)\nChoose a value", ((num_cpus::get() as f32) * 0.3).ceil() as i32)? as usize
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
                64,
            )?
        } else {
            64
        }
    }

    // Causes palette spread
    if args.qfactor < 0 {
        args.qfactor = if args.ask {
            prompt_number(
                Range { start: 4, end: 128 },
                "\nEnter a qfactor\nChoose a value",
                64,
            )? as i32
        } else {
            0
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
    let name = format!(
        "output/{}_{}_f{}-p{}.{}",
        outname,
        temp[temp.len() - 1],
        args.fsize,
        args.psize,
        ext
    );

    println!(
        "Processing: {}.{}, with palette: {}, at img size: {}, and palette size: {}",
        &fname, &ext, &pname, &args.fsize, &args.psize
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
            args.qfactor as f32,
        )?;
        image.save(name)?;
    } else {
        // let palette = generate_mosaic_palette(pname, psize);
    }
    println!("Finished in: {}ms", now.elapsed().as_millis());
    Ok(())
}
