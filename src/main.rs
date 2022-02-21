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

use glob::glob;
use image::{imageops::FilterType::Lanczos3, ImageBuffer, Rgba};
use ndarray::Array1;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::path::{self, PathBuf};
use std::time::Instant;
use std::{cmp::max, env, fs::read_dir, path::Path};

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

fn get_palette_dimensions(pname: &str) -> (usize, usize) {
    let palette_path = format!("palettes/{}", pname);
    let (mut w, mut h) = (0, 0);
    if Path::new(&palette_path).is_dir() {
        let images_paths = format!("{}/*", &palette_path);
        let image = image::open(glob(&images_paths).expect("Error").nth(0).unwrap().unwrap())
            .unwrap()
            .into_rgba8();
        w = image.width() as usize;
        h = image.height() as usize;
    }
    (w, h)
}

fn resize_dims((mut w, mut h): (usize, usize), max_size: u32) -> (usize, usize) {
    let max_dimension = max(w, h) as f32;
    w = (((w as f32) / max_dimension) * (max_size as f32)) as usize;
    h = (((h as f32) / max_dimension) * (max_size as f32)) as usize;
    (w, h)
}

// Creates a Vec of the struct Label, which houses every
//      single image in the palette, with its computed color
fn generate_pixel_palette(pname: &str, max_size: u32) -> Vec<Label> {
    let (pw, ph) = resize_dims(get_palette_dimensions(&pname), max_size);

    let palette_path = format!("palettes/{}", pname);
    let mut palette: Vec<Label> = vec![
        Label {
            color: Array1::<f32>::zeros(4),
            image: ImageBuffer::<Rgba<u8>, Vec<u8>>::new(1, 1),
            count: 0,
            id: 0
        };
        1
    ];

    if Path::new(&palette_path).is_dir() {
        let set_count: usize = read_dir(&palette_path).unwrap().count() as usize;
        palette = vec![
            Label {
                color: Array1::<f32>::zeros(4),
                image: ImageBuffer::<Rgba<u8>, Vec<u8>>::new(pw as u32, ph as u32),
                count: 0,
                id: 0
            };
            set_count
        ];

        let mut tempfix = 0;
        let images_paths = format!("{}/*", &palette_path);
        for (i, item) in glob(&images_paths).expect("Error").enumerate() {
            let item_name = item.unwrap();
            if item_name.as_os_str().to_str().unwrap().contains("ini") == true {
                tempfix += 1;
            }
            if item_name.as_os_str().to_str().unwrap().contains("ini") == false {
                let image = image::open(&item_name)
                    .unwrap()
                    .resize(pw as u32, ph as u32, Lanczos3)
                    .into_rgba8();
                palette[i].color = compute_average_color(&image, pw, ph);
                palette[i].image = image;
                palette[i].id = i - tempfix;
            }
        }
    }
    palette
}

// Maps pixels to palette items
fn generate_image_pixel_mode(
    fname: &str,
    pname: &str,
    palette: &mut Vec<Label>,
    pmax: u32,
    imax: u32,
    qfactor: f32,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let image_path = format!("input/{}", fname);
    let mut output = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(1, 1);

    if Path::new(&image_path).is_file() {
        let image = image::open(&image_path).unwrap();
        // Get dimensions of input image and palette items
        let (w, h) = resize_dims((image.width() as usize, image.height() as usize), imax);
        let (pw, ph) = resize_dims(get_palette_dimensions(&pname), pmax);
        // Resize input image
        let image = image.resize(imax, imax, Lanczos3).into_rgba8();
        output = ImageBuffer::<Rgba<u8>, Vec<u8>>::new((pw * w) as u32, (ph * h) as u32);
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
    }
    output
}

fn input_assign(dir: &str) -> String {
    let mut var = String::from("");
    // Keep looping until we get sensible input
    while var == String::from("") {
        // This is our path
        let input_paths = format!("{}/*", dir);
        // Iterate over every item in folder
        println!("Please enter the number of the image you'd like to process:");
        for (i, item) in glob(&input_paths).unwrap().enumerate() {
            println!(
                "{}: {}",
                i,
                item.unwrap().to_str().expect("Invalid Image Name")
            );
        }
        // Take in input from the user
        let mut string = String::from("");
        std::io::stdin().read_line(&mut string).unwrap();

        // Only parse the line if it contains numbers
        if string.trim().chars().all(char::is_numeric) {
            let line = string.trim();
            let p: u32 = line.parse().unwrap();
            // Index into the chosen filepath
            let file_chosen = glob(&input_paths)
                .unwrap()
                .nth(p as usize)
                .unwrap()
                .unwrap();
            // Trim the filepath to exclude dir
            var = String::from(
                file_chosen
                    .to_str()
                    .unwrap()
                    .split(path::MAIN_SEPARATOR)
                    .collect::<Vec<&str>>()[1],
            );
            println!("You Chose: {}", var);
        }
    }
    var
}
fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut fname = String::from("");
    let mut pname = String::from("");
    let mut fsize = 256;
    let mut psize = 16;
    let pixel_mode = true;
    let mut qfactor = 64.0;
    let mut arg: &str = "";
    for (i, e) in args.iter().enumerate() {
        if i < args.len() - 1 {
            arg = Box::leak(args[i + 1].clone().into_boxed_str());
        }
        match e.as_str() {
            "-p" => pname = String::from(arg),
            "-f" => fname = String::from(arg),
            "-v" => qfactor = arg.parse::<f32>().expect("Invalid value for v"),
            "-fs" => fsize = arg.parse::<u32>().expect("Invalid value for fsize"),
            "-ps" => psize = arg.parse::<u32>().expect("Invalid value for psize"),
            _ => {}
        }
    }
    if fname == "" {
        fname = input_assign("input");
        println!();
    }
    println!();
    if pname == "" {
        pname = input_assign("palettes");
        println!();
    }

    println!(
        "Processing: {}, with palette: {}, at img size: {}, and palette size: {}",
        &fname, &pname, &fsize, &psize
    );
    let now = Instant::now();
    if pixel_mode {
        let mut palette = generate_pixel_palette(&pname, psize);
        let image = generate_image_pixel_mode(&fname, &pname, &mut palette, psize, fsize, qfactor);
        let save_name = format!(
            "output/{}-{}_p{}_f{}_v{:e}.{}",
            fname.split(".").collect::<Vec<&str>>()[0],
            pname,
            psize,
            fsize,
            qfactor,
            fname.split(".").collect::<Vec<&str>>()[1]
        );
        image.save(save_name).expect("Error");
    } else {
        // let palette = generate_mosaic_palette(pname, psize);
    }
    println!("Finished in: {}ms", now.elapsed().as_millis());
    Ok(())
}
