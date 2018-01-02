extern crate chrono;
extern crate docopt;
extern crate geo;
extern crate gpx;
extern crate image;
extern crate imageproc;
#[macro_use]
extern crate lazy_static;
extern crate palette;
extern crate rayon;
extern crate rusttype;
#[macro_use]
extern crate serde_derive;

use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path;

use docopt::Docopt;
use gpx::read;
use gpx::{Gpx, Track};
use geo::Point;
use palette::{Gradient, Rgb};
use image::ImageBuffer;
use rayon::prelude::*;
use rusttype::{FontCollection,Font, Scale, point, PositionedGlyph};


const USAGE: &'static str = "
Generate video from GPX files.

Usage:
  derivers [options] <top-lat> <left-lng> <bottom-lat> <right-lng> <width> <height> <directory>

Options:
";


#[derive(Debug, Deserialize)]
struct CommandArgs {
    arg_top_lat: f64,
    arg_left_lng: f64,
    arg_bottom_lat: f64,
    arg_right_lng: f64,
    arg_width: u32,
    arg_height: u32,
    arg_directory: String,
}


type ScreenPoint = (u32, u32);


lazy_static!{
    static ref GRADIENT: Gradient<Rgb<f64>> = {
        let stops = vec![
            (0, 0, 0),
            // (255, 69, 56),
        ].into_iter().map(|p| Rgb::new_u8(p.0, p.1, p.2));

        Gradient::new(stops)
    };

    static ref FONT_FACE: Font<'static> = {
        let font_data = include_bytes!("../fonts/Roboto-Light.ttf");
        let collection = FontCollection::from_bytes(font_data as &[u8]);

        collection.into_font().unwrap()
    };
}

struct Heatmap {
    top_left: Point<f64>,
    bottom_right: Point<f64>,
    width: u32,
    height: u32,
    heatmap: Vec<u32>,
    start: ScreenPoint,
    max_value: u32,
}


impl Heatmap {
    pub fn from(args: &CommandArgs) -> Heatmap {
        // h == w * (top - bottom) / (right - left)
        let ratio = (args.arg_top_lat - args.arg_bottom_lat) /
            (args.arg_right_lng - args.arg_left_lng);

        let width = args.arg_width as u32;
        let height = ((args.arg_width as f64) * ratio) as u32;

        println!("Computed height: {:?}", height);

        let size = (width * height) as usize;

        let mut heatmap = Vec::with_capacity(size);
        for _ in 0..size {
            heatmap.push(0);
        }

        Heatmap {
            top_left: Point::new(args.arg_left_lng, args.arg_top_lat),
            bottom_right: Point::new(args.arg_right_lng, args.arg_bottom_lat),
            width: width,
            height: height,
            heatmap: heatmap,
            start: (0, 0),
            max_value: 0,
        }
    }

    pub fn as_image(&self, text: &str) -> image::DynamicImage {
        let mut color_map = self.heatmap
            .clone()
            .into_par_iter()
            .map(|count| {
                if count == 0 {
                    return (255, 255, 255);
                }

                let heat = (count as f64).log(self.max_value as f64);

                GRADIENT.get(heat).to_pixel()
            })
            .collect::<Vec<_>>();

        let size = (self.width * self.height * 3) as usize;
        let mut pixels = Vec::with_capacity(size);

        self.render_text_to_image(text, &mut color_map);

        for pxl in color_map.iter() {
            pixels.extend_from_slice(&[pxl.0, pxl.1, pxl.2]);
        }

        let buffer = ImageBuffer::from_raw(self.width, self.height, pixels).unwrap();
        image::ImageRgb8(buffer)
    }

    pub fn render_text_to_image(&self, text: &str, pixels: &mut Vec<(u8, u8, u8)>) {
        let scale = Scale::uniform(self.height as f32 / 20.0);
        let v_metrics = FONT_FACE.v_metrics(scale);

        let width = self.width as i32;
        let height = self.height as i32;

        let offset = rusttype::point(0.0, height as f32 - v_metrics.ascent);
        let glyphs: Vec<PositionedGlyph> = FONT_FACE.layout(text, scale, offset).collect();

        for g in glyphs {
            if let Some(bb) = g.pixel_bounding_box() {
                g.draw(|gx, gy, gv| {
                    let gx = gx as i32 + bb.min.x;
                    let gy = gy as i32 + bb.min.y;

                    let image_x = gx as i32;
                    let image_y = gy as i32;

                    if image_x >= 0 && image_x < width && image_y >= 0 && image_y < height {
                        let index = (image_x + (image_y * width)) as usize;
                        let color = 255 - (gv * 255.0) as u8;
                        pixels[index] = (color, color, color);
                    }
                })
            }
        }
    }

    pub fn save_frame<W: Write>(&self, writer: &mut W, fmt: image::ImageFormat, text: &str) {
        let image = self.as_image(text);
        image.save(writer, fmt).unwrap();
    }

    #[inline]
    fn get_pixel_mut(&mut self, point: &ScreenPoint) -> Option<&mut u32> {
        if point.0 >= self.width || point.1 >= self.height {
            return None;
        }

        let index = (point.0 + (point.1 * self.width)) as usize;
        Some(&mut self.heatmap[index])
    }

    #[inline]
    pub fn add_point(&mut self, point: &ScreenPoint) {

        // FIXME: lol rust?
        let px = {
            let px = self.get_pixel_mut(point).unwrap();
            *px += 1;
            *px
        };

        self.max_value = self.max_value.max(px);
    }

    #[allow(dead_code)]
    pub fn decay(&mut self, amount: u32) {
        self.max_value -= 1;

        self.heatmap.par_iter_mut().for_each(|px| if *px > amount {
            *px -= amount;
        });
    }

    // Using simple equirectangular projection for now. Returns None if point
    // is off screen.
    pub fn project_to_screen(&self, coord: &Point<f64>) -> Option<ScreenPoint> {
        // lng is x pos
        let x_pos = self.top_left.lng() - coord.lng();
        let y_pos = self.top_left.lat() - coord.lat();

        let x_offset = x_pos / (self.top_left.lng() - self.bottom_right.lng());
        let y_offset = y_pos / (self.top_left.lat() - self.bottom_right.lat());

        let (x, y) = (
            (x_offset * self.width as f64),
            (y_offset * self.height as f64),
        );

        if (x < 0.0 || x as u32 >= self.width) || (y < 0.0 || y as u32 >= self.height) {
            None
        } else {
            Some((x as u32, y as u32))
        }
    }
}


#[derive(Debug)]
struct Activity {
    name: String,
    date: chrono::DateTime<chrono::Utc>,
    track_points: Vec<Point<f64>>,
}

fn parse_gpx(path: &path::PathBuf) -> Result<Activity, Box<Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let gpx: Gpx = read(reader)?;

    // Nothing to do if there are no tracks
    if gpx.tracks.len() == 0 {
        return Err(Box::from("file has no tracks"));
    } else if gpx.tracks.len() > 1 {
        println!("Warning! more than 1 track, just taking first");
    }

    let track: &Track = &gpx.tracks[0];

    let mut activity = Activity {
        name: track.name.clone().unwrap_or(String::from("Untitled")),
        date: chrono::Utc::now(),
        track_points: vec![],
    };

    if let Some(metadata) = gpx.metadata {
        if let Some(time) = metadata.time {
            activity.date = time;
        }
    }

    // Append all the waypoints.
    for seg in track.segments.iter() {
        let points = seg.points.iter().map(|ref wpt| wpt.point());
        activity.track_points.extend(points);
    }

    if activity.track_points.len() == 0 {
        Err(Box::from("No track points"))
    } else {
        Ok(activity)
    }
}


fn main() {
    let args: CommandArgs = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    println!("{:?}", args);

    let mut map = Heatmap::from(&args);

    let paths: Vec<path::PathBuf> = fs::read_dir(args.arg_directory)
        .unwrap()
        .into_iter()
        .map(|p| p.unwrap().path())
        .collect();

    print!("Parsing {:?} GPX files...", paths.len());

    let mut activities: Vec<Activity> = paths
        .into_par_iter()
        .filter_map(|ref p| parse_gpx(p).ok())
        .collect();

    activities.sort_by_key(|a| a.date);

    println!("Done!");

    let ppm_file = &mut File::create("heatmap.ppm").unwrap();
    let png_file = &mut File::create("heatmap.png").unwrap();
    let mut counter;

    for act in activities {
        println!("Activity: {}", act.name);

        let pixels: Vec<ScreenPoint> = act.track_points
            .par_iter()
            .filter_map(|ref pt| map.project_to_screen(pt))
            .collect();

        counter = 0;
        for ref point in pixels.into_iter() {
            // map.start = point.clone();
            map.add_point(point);

            counter += 1;

            if counter % (5 * 150) == 0 {
                map.save_frame(ppm_file, image::PPM, act.name.as_str());
            }
        }

        // FIXME: this is pretty ugly.
        // map.decay(1);
    }

    map.save_frame(ppm_file, image::PPM, "");
    map.save_frame(png_file, image::PNG, "");
}
