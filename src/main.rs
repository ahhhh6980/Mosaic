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

use std::{path::Path, fs::read_dir, env};
use image::{ImageBuffer, Rgba, imageops::FilterType::Lanczos3};
use glob::glob;
use ndarray::{Array1};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::time::{Instant};

#[derive(Debug, Clone)]
struct Label {
    color: Array1<f32>,
    image: ImageBuffer<Rgba<u8>, Vec<u8>>
}

fn compute_average_color(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, w: usize, h: usize) -> Array1<f32> {
    let mut avg = [0f32;4];
    for (i, e) in img.iter().enumerate() {  avg[i % 4] += *e as f32 }
    for i in 0..4 { avg[i] /=  h as f32 * w as f32; }
    Array1::from(avg.to_vec())
}

fn generate_palette(pname: &str, pw: u32, ph: u32) -> Vec<Label> {
    let palette_path = format!("palettes/{}", pname);
    let mut palette: Vec<Label> = vec![Label{color: Array1::<f32>::zeros(4), image: ImageBuffer::<Rgba<u8>, Vec<u8>>::new(1, 1)};1];
    if Path::new(&palette_path).is_dir() {
    
        let set_count: usize = read_dir(&palette_path).unwrap().count() as usize;
        palette = vec![Label{color: Array1::<f32>::zeros(4), image: ImageBuffer::<Rgba<u8>, Vec<u8>>::new(pw as u32, ph as u32)};set_count];

        let images_paths = format!("{}/*", &palette_path);
        for (i, item) in glob(&images_paths).expect("Error").enumerate() {
            let item_name = item.unwrap();
            let image = image::open(&item_name).unwrap().into_rgba8();
            palette[i].color = compute_average_color(&image, pw as usize, ph as usize);
            palette[i].image = image;
        }
    }
    palette
}

fn bad_color_distance(pixel_a: &Array1<f32>, pixel_b: &Array1<f32>) -> u64 {
    let c1 = pixel_a / 256.0;
    let c2 = pixel_b / 256.0;
    let dc = &c2 - &c1;
    let r = (c1[0] + c2[0]) / 2.0;
    let dr = (2.0 + (r / 256.0)) * dc[0] * dc[0];
    let dg = 4.0 * dc[1] * dc[1];
    let db = (2.0 + ((255.0 - r) / 256.0)) * dc[2] * dc[2];
    ((dr + dg + db) * 1024.0) as u64
}

fn find_closest_image(pixel: &Array1<f32>, palette: &Vec<Label>) -> (ImageBuffer<Rgba<u8>, Vec<u8>>, Array1<f32>) {
    let label = palette.par_iter().min_by_key(|x| bad_color_distance(&pixel, &x.color)).unwrap();
    (label.image.clone(), label.color.clone())
}

fn generate_image(fname: &str, palette: &Vec<Label>, pw: u32, ph: u32, dw: u32, dh: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let image_path = format!("input/{}", fname);
    let mut output = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(1,1);
    if Path::new(&image_path).is_file() {
        let image = image::open(&image_path).unwrap().resize(dw, dh, Lanczos3).into_rgba8();
        let (w, h) = (image.width(), image.height());
        output = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(pw*w,ph*h);
        for y in 0..h {
            for x in 0..w {
                let temp = (image[(x,y)].0).to_vec();
                let pixel: Vec<f32> = temp.iter().map(|v| *v as f32).collect();
                let pixel = Array1::<f32>::from(pixel);
                let (pimage, pcolor) = find_closest_image(&pixel, &palette);
                for oy in 0..ph {
                    for ox in 0..pw {
                        let mut ppixel = pimage[(ox,oy)];
                        if ppixel[3] < 128 {
                            let value: u16 = pcolor.iter().map(|v| *v as u16).sum();
                            ppixel[0] = (value / 4) as u8;
                            ppixel[1] = (value / 4) as u8;
                            ppixel[2] = (value / 4) as u8;
                        }
                        ppixel[3] = pixel[3] as u8;
                        output.put_pixel(x * pw + ox, y * ph + oy, ppixel);
                    }
                }
            }
        }
    }
    output
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut fname = "test.tiff";
    let mut pname = "emoji";
    let mut fsize = 128;
    let mut psize = 72;
    for e in args.iter() {
        if e == "-p" {  pname = e;  }
        if e == "-f" {  fname = e;  }
        if e == "-fs"{  fsize = e.parse::<u32>().unwrap();   }
        if e == "-ps"{  psize = e.parse::<u32>().unwrap();   }
    }
    println!("Processing: {}, with palette: {}, at img size: {}, and palette size: {}", &fname, &pname, &fsize, &psize);
    let now = Instant::now();
    let palette = generate_palette(pname, psize, psize);
    let image = generate_image(fname, &palette, psize, psize, fsize, fsize);
    //dbg!(image);
    let save_name = format!("output/{}_p{}_f{}.{}", fname.split(".").collect::<Vec<&str>>()[0], psize, fsize, fname.split(".").collect::<Vec<&str>>()[1]);
    image.save(save_name).unwrap();
    println!("Finished in: {}ms", now.elapsed().as_millis());
    Ok(())
}
