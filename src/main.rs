#[macro_use]
extern crate serde_derive;
extern crate docopt;
extern crate image;
extern crate gpx;
extern crate geo;
extern crate chrono;
extern crate rayon;

use std::fs;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path;

use docopt::Docopt;
use gpx::read;
use gpx::{Gpx, Track};
use geo::Point;
use image::ImageBuffer;
use rayon::prelude::*;


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

struct Heatmap {
    top_left: Point<f64>,
    bottom_right: Point<f64>,
    width: u32,
    height: u32,
    heatmap: image::DynamicImage,
}

impl Heatmap {
    pub fn from(args: &CommandArgs) -> Heatmap {
        // h == w * (top - bottom) / (right - left)
        let ratio = (args.arg_top_lat - args.arg_bottom_lat) /
            (args.arg_right_lng - args.arg_left_lng);

        let width = args.arg_width as u32;
        let height = ((args.arg_width as f64) * ratio) as u32;

        println!("Computed height: {:?}", height);

        let buffer = ImageBuffer::from_pixel(width, height, image::Rgb([255, 255, 255]));
        let heatmap = image::ImageRgb8(buffer);

        Heatmap {
            top_left: Point::new(args.arg_left_lng, args.arg_top_lat),
            bottom_right: Point::new(args.arg_right_lng, args.arg_bottom_lat),
            width: width,
            height: height,
            heatmap: heatmap,
        }
    }

    pub fn save_frame<W: Write>(&self, writer: &mut W) {
        self.heatmap.save(writer, image::PPM).unwrap();
    }

    #[inline]
    pub fn add_point(&mut self, point: &ScreenPoint) {
        let image = self.heatmap.as_mut_rgb8().unwrap();
        let pixel = image.get_pixel_mut(point.0, point.1);

        let c = if pixel[0] == 0 {
            pixel[0]
        } else {
            pixel[0] - 15
        };

        *pixel = image::Rgb([c; 3]);
    }

    pub fn decay(&mut self, amount: u8) {
        let image = self.heatmap.as_mut_rgb8().unwrap();
        for (_x, _y, pixel) in image.enumerate_pixels_mut() {
            if pixel[0] < 255 - amount {
                *pixel = image::Rgb([pixel[0]; 3]);
            }
        }
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

fn parse_gpx(path: &path::PathBuf) -> Option<Activity> {
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);

    let gpx: Gpx = read(reader).unwrap();

    // Nothing to do if there are no tracks
    if gpx.tracks.len() == 0 {
        return None;
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
        None
    } else {
        Some(activity)
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

    let mut activities: Vec<Activity> = paths
        .into_par_iter()
        .filter_map(|ref p| parse_gpx(p))
        .collect();

    activities.sort_by_key(|a| a.date);

    let fout = &mut File::create("heatmap.ppm").unwrap();
    let mut counter = 0;

    for act in activities {
        println!("Activity: {:?}", act.name);

        let pixels: Vec<ScreenPoint> = act.track_points
            .into_iter()
            .filter_map(|ref pt| map.project_to_screen(pt))
            .collect();

        for ref point in pixels.into_iter() {
            map.add_point(point);
            counter += 1;

            if counter == 150 {
                map.save_frame(fout);
                counter = 0;
            }
        }

        // FIXME: this is pretty ugly.
        // map.decay(1);
    }
}
